use std::sync::Arc;

use tokio::sync::mpsc;

use crate::app::{CitingPaper, PaperEntry, PaperMeta, MetaFetchStatus, SourceKind, SourceStatus};
use crate::config::{NewsFeedConfig, Profile, SocialFeedConfig};
use crate::llm::{self, ChatMessage, LlmProvider};
use crate::providers;
use crate::providers::huggingface::HfSpotlight;
use crate::providers::news::NewsArticle;
use crate::providers::social::SocialPost;

// ── Channel Messages ─────────────────────────────────────────

pub enum NetworkAction {
    FetchFeed(Profile),
    FetchHfSpotlight,
    FetchPaperMeta(String, bool), // arxiv_id, enable_s2
    FetchSocialFeed(Vec<SocialFeedConfig>, String),
    FetchNewsFeed(Vec<NewsFeedConfig>),
    FetchNewsArticle(String), // url
    Summarize {
        arxiv_id: String,
        mode: String,
        abstract_text: String,
        provider_idx: usize,
    },
    GenerateScaffold {
        arxiv_id: String,
        title: String,
        abstract_text: String,
        provider_idx: usize,
    },
    FetchFullText(String), // arxiv_id
    SearchPapers(String),  // query
}

pub enum NetworkEvent {
    FeedLoaded(Vec<PaperEntry>),
    HfSpotlightLoaded(HfSpotlight),
    PaperMetaLoaded { arxiv_id: String, meta: PaperMeta },
    SocialFeedLoaded(Vec<SocialPost>),
    NewsFeedLoaded(Vec<NewsArticle>),
    NewsArticleLoaded { url: String, markdown: String },
    SummaryLoaded { arxiv_id: String, mode: String, text: String },
    ScaffoldLoaded { arxiv_id: String, text: String },
    FullTextLoaded { arxiv_id: String, text: String },
    SearchResultsLoaded(Vec<PaperEntry>),
    SourceProgress { kind: SourceKind, name: String, status: SourceStatus },
    Error(String),
}

// ── Background Worker ────────────────────────────────────────

pub fn spawn_worker(
    mut action_rx: mpsc::Receiver<NetworkAction>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    llm_providers: Arc<Vec<Box<dyn LlmProvider>>>,
) {
    tokio::spawn(async move {
        let client = providers::build_client();
        let llm_client = providers::build_llm_client();

        while let Some(action) = action_rx.recv().await {
            let client = client.clone();
            let llm_client = llm_client.clone();
            let tx = event_tx.clone();
            let providers = llm_providers.clone();

            // Inner-spawn each action so a slow fetch doesn't block others
            tokio::spawn(async move {
                match action {
                    NetworkAction::FetchFeed(profile) => {
                        let _ = tx.send(NetworkEvent::SourceProgress {
                            kind: SourceKind::Papers,
                            name: profile.name.clone(),
                            status: SourceStatus::Started,
                        });
                        let max_results = 50;
                        let result = providers::arxiv::fetch_papers(
                            &client,
                            &profile.arxiv_categories,
                            max_results,
                        )
                        .await;
                        let status = match &result {
                            Ok(p) if !p.is_empty() => SourceStatus::Done,
                            _ => SourceStatus::Failed,
                        };
                        let _ = tx.send(NetworkEvent::SourceProgress {
                            kind: SourceKind::Papers,
                            name: profile.name.clone(),
                            status,
                        });
                        match result {
                            Ok(papers) if !papers.is_empty() => {
                                let _ = tx.send(NetworkEvent::FeedLoaded(papers));
                            }
                            Ok(_) => {
                                let _ = tx.send(NetworkEvent::Error(
                                    "ArXiv returned no papers for this profile.".into(),
                                ));
                                let _ = tx.send(NetworkEvent::FeedLoaded(Vec::new()));
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(
                                    format!("Feed fetch failed: {}", e),
                                ));
                                let _ = tx.send(NetworkEvent::FeedLoaded(Vec::new()));
                            }
                        }
                    }

                    NetworkAction::FetchHfSpotlight => {
                        let _ = tx.send(NetworkEvent::SourceProgress {
                            kind: SourceKind::HfSpotlight,
                            name: "Daily Paper".into(),
                            status: SourceStatus::Started,
                        });
                        let result = providers::huggingface::fetch_spotlight(&client).await;
                        let status = match &result {
                            Ok(_) => SourceStatus::Done,
                            Err(_) => SourceStatus::Failed,
                        };
                        let _ = tx.send(NetworkEvent::SourceProgress {
                            kind: SourceKind::HfSpotlight,
                            name: "Daily Paper".into(),
                            status,
                        });
                        match result {
                            Ok(spotlight) => {
                                let _ = tx.send(NetworkEvent::HfSpotlightLoaded(spotlight));
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(format!("HF: {}", e)));
                            }
                        }
                    }

                    NetworkAction::FetchPaperMeta(arxiv_id, enable_s2) => {
                        let hf_client = client.clone();
                        let hf_id = arxiv_id.clone();

                        // Always fetch HF metadata
                        let hf_result = providers::hf_papers::fetch_paper_meta(&hf_client, &hf_id).await;

                        // Optionally fetch S2 citations
                        let s2_result = if enable_s2 {
                            let s2_client = client.clone();
                            let s2_id = arxiv_id.clone();
                            Some(providers::semantic_scholar::fetch_paper_meta(&s2_client, &s2_id).await)
                        } else {
                            None
                        };

                        let mut meta = PaperMeta {
                            citation_count: 0,
                            influential_count: 0,
                            top_citations: Vec::new(),
                            repo_url: None,
                            repo_stars: None,
                            upvotes: 0,
                            ai_summary: None,
                            ai_keywords: Vec::new(),
                            num_comments: 0,
                            project_page: None,
                            submitted_by: None,
                            published_at: None,
                            s2_found: false,
                            meta_status: MetaFetchStatus::Loaded,
                        };

                        let mut s2_ok = false;

                        if let Some(s2_result) = s2_result {
                            match s2_result {
                                Ok(s2) => {
                                    meta.citation_count = s2.citation_count;
                                    meta.influential_count = s2.influential_count;
                                    meta.s2_found = s2.found;
                                    meta.top_citations = s2
                                        .top_citations
                                        .into_iter()
                                        .map(|c| CitingPaper {
                                            title: c.title,
                                            citation_count: c.citation_count,
                                        })
                                        .collect();
                                    s2_ok = true;
                                }
                                Err(e) => {
                                    let _ = tx.send(NetworkEvent::Error(format!("S2: {}", e)));
                                }
                            }
                        }

                        let mut hf_ok = false;

                        match hf_result {
                            Ok(hf) => {
                                meta.repo_url = hf.repo_url;
                                meta.repo_stars = hf.repo_stars;
                                meta.upvotes = hf.upvotes;
                                meta.ai_summary = hf.ai_summary;
                                meta.ai_keywords = hf.ai_keywords;
                                meta.num_comments = hf.num_comments;
                                meta.project_page = hf.project_page;
                                meta.submitted_by = hf.submitted_by;
                                meta.published_at = hf.published_at;
                                hf_ok = true;
                            }
                            Err(_) => {}
                        }

                        // Only mark as failed if both S2 and HF failed
                        if !s2_ok && !hf_ok {
                            meta.meta_status = MetaFetchStatus::Failed;
                        }

                        let _ = tx.send(NetworkEvent::PaperMetaLoaded { arxiv_id, meta });
                    }

                    NetworkAction::FetchSocialFeed(feeds, nitter) => {
                        let mut all_posts = Vec::new();
                        for feed in &feeds {
                            let _ = tx.send(NetworkEvent::SourceProgress {
                                kind: SourceKind::Social,
                                name: feed.name.clone(),
                                status: SourceStatus::Started,
                            });
                            let result = providers::social::fetch_one(&client, feed, &nitter).await;
                            let status = match &result {
                                Ok(_) => SourceStatus::Done,
                                Err(_) => SourceStatus::Failed,
                            };
                            let _ = tx.send(NetworkEvent::SourceProgress {
                                kind: SourceKind::Social,
                                name: feed.name.clone(),
                                status,
                            });
                            if let Ok(posts) = result {
                                all_posts.extend(posts);
                            }
                        }
                        all_posts.sort_by(|a, b| b.published.cmp(&a.published));
                        let _ = tx.send(NetworkEvent::SocialFeedLoaded(all_posts));
                    }

                    NetworkAction::FetchNewsFeed(feeds) => {
                        let mut all_articles = Vec::new();
                        for feed in &feeds {
                            let _ = tx.send(NetworkEvent::SourceProgress {
                                kind: SourceKind::News,
                                name: feed.name.clone(),
                                status: SourceStatus::Started,
                            });
                            let result = providers::news::fetch_one(&client, feed).await;
                            let status = match &result {
                                Ok(_) => SourceStatus::Done,
                                Err(_) => SourceStatus::Failed,
                            };
                            let _ = tx.send(NetworkEvent::SourceProgress {
                                kind: SourceKind::News,
                                name: feed.name.clone(),
                                status,
                            });
                            if let Ok(articles) = result {
                                all_articles.extend(articles);
                            }
                        }
                        all_articles.sort_by(|a, b| b.published.cmp(&a.published));
                        let _ = tx.send(NetworkEvent::NewsFeedLoaded(all_articles));
                    }

                    NetworkAction::FetchNewsArticle(url) => {
                        match providers::news::fetch_article_markdown(&client, &url).await {
                            Ok(markdown) => {
                                let _ = tx.send(NetworkEvent::NewsArticleLoaded { url, markdown });
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(format!("News article: {}", e)));
                            }
                        }
                    }

                    NetworkAction::Summarize { arxiv_id, mode, abstract_text, provider_idx } => {
                        if let Some(provider) = providers.get(provider_idx) {
                            debug_log!("LLM summary: {} mode={} provider={}", arxiv_id, mode, provider.name());
                            let system = llm::summary_system_prompt(&mode);
                            let messages = vec![
                                ChatMessage::system(system),
                                ChatMessage::user(abstract_text),
                            ];
                            match provider.chat(&llm_client, &messages, 500).await {
                                Ok(text) => {
                                    debug_log!("LLM summary done: {} ({}B)", arxiv_id, text.len());
                                    let _ = tx.send(NetworkEvent::SummaryLoaded {
                                        arxiv_id,
                                        mode,
                                        text,
                                    });
                                }
                                Err(e) => {
                                    debug_log!("LLM summary error: {}", e);
                                    let _ = tx.send(NetworkEvent::Error(format!("LLM: {}", e)));
                                }
                            }
                        } else {
                            debug_log!("LLM summary: no provider at idx {}", provider_idx);
                        }
                    }

                    NetworkAction::GenerateScaffold { arxiv_id, title, abstract_text, provider_idx } => {
                        if let Some(provider) = providers.get(provider_idx) {
                            debug_log!("LLM scaffold: {} provider={}", arxiv_id, provider.name());
                            let system = llm::scaffold_system_prompt().to_string();
                            let user_msg = format!("Paper: {}\n\nAbstract: {}", title, abstract_text);
                            let messages = vec![
                                ChatMessage::system(system),
                                ChatMessage::user(user_msg),
                            ];
                            match provider.chat(&llm_client, &messages, 4000).await {
                                Ok(text) => {
                                    debug_log!("LLM scaffold done: {} ({}B)", arxiv_id, text.len());
                                    let _ = tx.send(NetworkEvent::ScaffoldLoaded { arxiv_id, text });
                                }
                                Err(e) => {
                                    debug_log!("LLM scaffold error: {}", e);
                                    let _ = tx.send(NetworkEvent::Error(format!("LLM scaffold: {}", e)));
                                }
                            }
                        } else {
                            debug_log!("LLM scaffold: no provider at idx {}", provider_idx);
                        }
                    }

                    NetworkAction::FetchFullText(arxiv_id) => {
                        match providers::arxiv_html::fetch_full_text(&client, &arxiv_id).await {
                            Ok(Some(text)) => {
                                let _ = tx.send(NetworkEvent::FullTextLoaded { arxiv_id, text });
                            }
                            Ok(None) => {
                                let _ = tx.send(NetworkEvent::Error(
                                    "No HTML version available for this paper".into(),
                                ));
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(format!("Full text: {}", e)));
                            }
                        }
                    }

                    NetworkAction::SearchPapers(query) => {
                        match providers::hf_search::search_papers(&client, &query).await {
                            Ok(papers) => {
                                let _ = tx.send(NetworkEvent::SearchResultsLoaded(papers));
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(format!("Search: {}", e)));
                            }
                        }
                    }
                }
            });
        }
    });
}

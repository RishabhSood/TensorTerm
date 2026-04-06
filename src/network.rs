use std::sync::Arc;

use tokio::sync::mpsc;

use crate::app::{CitingPaper, PaperEntry, PaperMeta, MetaFetchStatus};
use crate::config::{Profile, SocialFeedConfig};
use crate::llm::{self, ChatMessage, LlmProvider};
use crate::providers;
use crate::providers::huggingface::HfSpotlight;
use crate::providers::social::SocialPost;

// ── Channel Messages ─────────────────────────────────────────

pub enum NetworkAction {
    FetchFeed(Profile),
    FetchHfSpotlight,
    FetchPaperMeta(String, bool), // arxiv_id, enable_s2
    FetchSocialFeed(Vec<SocialFeedConfig>, String),
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
}

pub enum NetworkEvent {
    FeedLoaded(Vec<PaperEntry>),
    HfSpotlightLoaded(HfSpotlight),
    PaperMetaLoaded { arxiv_id: String, meta: PaperMeta },
    SocialFeedLoaded(Vec<SocialPost>),
    SummaryLoaded { arxiv_id: String, mode: String, text: String },
    ScaffoldLoaded { arxiv_id: String, text: String },
    FullTextLoaded { arxiv_id: String, text: String },
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
                        let max_results = 50;
                        match providers::arxiv::fetch_papers(
                            &client,
                            &profile.arxiv_categories,
                            max_results,
                        )
                        .await
                        {
                            Ok(papers) if !papers.is_empty() => {
                                let _ = tx.send(NetworkEvent::FeedLoaded(papers));
                            }
                            Ok(_) => {
                                // Empty result — fall back to mock
                                let _ = tx.send(NetworkEvent::Error(
                                    "ArXiv returned no papers, using mock data".into(),
                                ));
                                let papers = mock_papers_for_profile(&profile);
                                let _ = tx.send(NetworkEvent::FeedLoaded(papers));
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(e));
                                let papers = mock_papers_for_profile(&profile);
                                let _ = tx.send(NetworkEvent::FeedLoaded(papers));
                            }
                        }
                    }

                    NetworkAction::FetchHfSpotlight => {
                        match providers::huggingface::fetch_spotlight(&client).await {
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
                        match providers::social::fetch_social_feeds(&client, &feeds, &nitter).await {
                            Ok(posts) => {
                                let _ = tx.send(NetworkEvent::SocialFeedLoaded(posts));
                            }
                            Err(e) => {
                                let _ = tx.send(NetworkEvent::Error(format!("Social: {}", e)));
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
                }
            });
        }
    });
}

// ── Mock Data (ArXiv fallback) ──────────────────────────────

fn mock_papers_for_profile(profile: &Profile) -> Vec<PaperEntry> {
    let is_rl = profile.arxiv_categories.iter().any(|c| c == "cs.AI");

    if is_rl {
        vec![
            p("Multi-Agent PPO with Shared Value Decomposition",
              "Sunehag, P. et al.", "2026-04-04", "MARL",
              "2604.01001",
              "We extend PPO to the multi-agent setting via a shared \
               value decomposition network, achieving SOTA on StarCraft \
               micromanagement benchmarks without centralized critics."),
            p("TD3 with Hindsight Experience Replay Revisited",
              "Andrychowicz, M. et al.", "2026-04-02", "RL",
              "2604.00892",
              "Combining TD3's clipped double-Q with hindsight relabeling \
               yields 40% faster convergence on sparse-reward robotics tasks."),
            p("Offline RL via Conservative Q-Learning at Scale",
              "Kumar, A. et al.", "2026-03-30", "RL",
              "2603.18221",
              "Scaling CQL to 10B-parameter transformers on D4RL datasets, \
               demonstrating that offline RL benefits from the same scaling \
               laws observed in language modeling."),
            p("DDPG-Based Continuous Control in MuJoCo Envs",
              "Lillicrap, T. et al.", "2026-03-27", "RL",
              "2603.16110",
              "A modernized DDPG baseline with layer normalization and \
               distributional critics that matches SAC on continuous \
               control without entropy tuning."),
            p("Decision Transformer Meets World Models",
              "Chen, L. et al.", "2026-03-22", "RL",
              "2603.13882",
              "Integrating a learned latent world model into the Decision \
               Transformer architecture for model-based offline planning \
               in Atari and MuJoCo."),
        ]
    } else {
        vec![
            p("Attention Is All You Need v2: Sparse Mixture Routing",
              "Vaswani, A. et al.", "2026-04-05", "NLP",
              "2604.02100",
              "Replacing dense attention with a top-k sparse mixture of \
               attention heads reduces FLOPs by 60% while matching dense \
               Transformer quality on WMT and GLUE."),
            p("TimesFM: Foundation Model for Time-Series",
              "Google Research", "2026-04-03", "Forecasting",
              "2604.01500",
              "A decoder-only foundation model pre-trained on 100B real-world \
               time points achieves zero-shot forecasting rivaling supervised \
               baselines across energy, retail, and finance domains."),
            p("Generative Flow Networks for Discrete Optimization",
              "Bengio, Y. et al.", "2026-04-01", "GFlowNets",
              "2604.00330",
              "GFlowNets trained with trajectory balance learn diverse, \
               high-reward solutions to combinatorial optimization problems \
               including molecular design and max-SAT."),
            p("RLHF Without Reward Models: Direct Preference Opt.",
              "Rafailov, R. et al.", "2026-03-28", "RL/NLP",
              "2603.17220",
              "DPO eliminates the reward model entirely by optimizing a \
               closed-form policy objective directly on preference pairs, \
               matching RLHF with 3x less compute."),
            p("Mamba-2: Linear-Time Sequence Modeling at Scale",
              "Gu, A., Dao, T.", "2026-03-25", "Arch",
              "2603.15400",
              "A structured state-space model with hardware-aware selective \
               scan kernels achieving Transformer-level quality at linear \
               time complexity on sequences up to 1M tokens."),
        ]
    }
}

fn p(title: &str, authors: &str, date: &str, domain: &str,
     arxiv_id: &str, abstract_text: &str) -> PaperEntry {
    PaperEntry {
        title: title.into(),
        authors: authors.into(),
        date: date.into(),
        domain: domain.into(),
        arxiv_id: Some(arxiv_id.into()),
        abstract_text: Some(abstract_text.into()),
        pdf_url: Some(format!("https://arxiv.org/pdf/{}", arxiv_id)),
    }
}

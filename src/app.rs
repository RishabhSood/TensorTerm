use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::config::{Config, Profile};
use crate::llm::LlmProvider;
use crate::network::{NetworkAction, NetworkEvent};
use crate::providers::huggingface::HfSpotlight;
use crate::providers::social::SocialPost;
use crate::scaffold_index::ScaffoldIndex;

// ── Enums ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Feed,
    Highlight,
    Article,
}

impl ActivePane {
    pub fn next(self) -> Self {
        match self {
            Self::Feed => Self::Highlight,
            Self::Highlight => Self::Article,
            Self::Article => Self::Feed,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Feed => Self::Article,
            Self::Highlight => Self::Feed,
            Self::Article => Self::Highlight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Help,
    Filter,
    Confirm,
    ScaffoldPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    ScrollDown,
    ScrollUp,
    NextPane,
    PrevPane,
    GoToTop,
    GoToBottom,
    SpawnImplementation,
    ExportToObsidian,
    OpenInBrowser,
    CycleProfile,
    RefreshFeed,
    ToggleHelp,
    Dismiss,
    ToggleFeedMode,
    StartFilter,
    CycleSort,
    CycleTimeWindow,
    CycleMaxItems,
    CycleSummaryMode,
    GenerateSummary,
    CycleProvider,
    ConfirmYes,
    ConfirmNo,
    ScaffoldInput(char),
    ScaffoldBackspace,
    ScaffoldConfirm,
    FilterInput(char),
    FilterBackspace,
    ConfirmFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedMode {
    Papers,
    Social,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSort {
    Date,
    Citations,
    Title,
}

impl PaperSort {
    pub fn next(self) -> Self {
        match self {
            Self::Date => Self::Citations,
            Self::Citations => Self::Title,
            Self::Title => Self::Date,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Citations => "citations",
            Self::Title => "title",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeWindow {
    Day,
    Week,
    Month,
    All,
}

impl TimeWindow {
    pub fn next(self) -> Self {
        match self {
            Self::Day => Self::Week,
            Self::Week => Self::Month,
            Self::Month => Self::All,
            Self::All => Self::Day,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "24h",
            Self::Week => "7d",
            Self::Month => "30d",
            Self::All => "all",
        }
    }
    /// Return the cutoff date as "YYYY-MM-DD" string, or None for All.
    pub fn cutoff_date(self) -> Option<String> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let days = match self {
            Self::Day => 1u64,
            Self::Week => 7,
            Self::Month => 30,
            Self::All => return None,
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff = now.saturating_sub(days * 86400);
        // Convert epoch seconds to YYYY-MM-DD
        let days_since_epoch = (cutoff / 86400) as i64;
        let (y, m, d) = days_to_ymd(days_since_epoch);
        Some(format!("{:04}-{:02}-{:02}", y, m, d))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryMode {
    Off,
    HfTldr,
    Eli5,
    Technical,
    KeyFindings,
    ResearchGaps,
}

impl SummaryMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::HfTldr,
            Self::HfTldr => Self::Eli5,
            Self::Eli5 => Self::Technical,
            Self::Technical => Self::KeyFindings,
            Self::KeyFindings => Self::ResearchGaps,
            Self::ResearchGaps => Self::Off,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::HfTldr => "TL;DR",
            Self::Eli5 => "ELI5",
            Self::Technical => "Technical",
            Self::KeyFindings => "Key Findings",
            Self::ResearchGaps => "Research Gaps",
        }
    }
    pub fn api_key(self) -> &'static str {
        match self {
            Self::Off | Self::HfTldr => "",
            Self::Eli5 => "eli5",
            Self::Technical => "technical",
            Self::KeyFindings => "key_findings",
            Self::ResearchGaps => "research_gaps",
        }
    }
    pub fn needs_llm(self) -> bool {
        !matches!(self, Self::Off | Self::HfTldr)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LoadingTask {
    Feed,
    HfSpotlight,
    PaperMeta(String),
    SocialFeed,
    LlmSummary(String),
    LlmScaffold(String),
}

// ── Data Models ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PaperEntry {
    pub title: String,
    pub authors: String,
    pub date: String,
    pub domain: String,
    pub arxiv_id: Option<String>,
    pub abstract_text: Option<String>,
    pub pdf_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaFetchStatus {
    NotFetched,
    Loading,
    Loaded,
    Failed,
}

#[derive(Debug, Clone)]
pub struct CitingPaper {
    pub title: String,
    pub citation_count: u32,
}

#[derive(Debug, Clone)]
pub struct PaperMeta {
    pub citation_count: u32,
    pub influential_count: u32,
    pub top_citations: Vec<CitingPaper>,
    pub repo_url: Option<String>,
    pub repo_stars: Option<u32>,
    pub upvotes: u32,
    pub ai_summary: Option<String>,
    pub ai_keywords: Vec<String>,
    pub num_comments: u32,
    pub project_page: Option<String>,
    pub submitted_by: Option<String>,
    pub published_at: Option<String>,
    pub s2_found: bool,
    pub meta_status: MetaFetchStatus,
}

// ── App State ────────────────────────────────────────────────

/// Convert days since Unix epoch to (year, month, day).
pub fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Normalize various date formats to "YYYY-MM-DD" for comparison.
/// Handles: "YYYY-MM-DD...", "Sat, 05 Apr 2025 12:00:00", "05 Apr 2025", etc.
fn normalize_date_to_iso(date_str: &str) -> String {
    let s = date_str.trim();

    // Already ISO-8601: "2025-04-05..." → take first 10
    if s.len() >= 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' {
        return s[..10].to_string();
    }

    // RFC 2822: "Sat, 05 Apr 2025 12:00:00" or "05 Apr 2025"
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    // Strip leading day-of-week if present (e.g., "Sat, ")
    let s = if s.len() > 5 && s[3..4].starts_with(',') {
        s[5..].trim()
    } else {
        s
    };

    // Try to parse "DD Mon YYYY" from the start
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 3 {
        if let Ok(day) = parts[0].parse::<u32>() {
            if let Some(month_idx) = months.iter().position(|&m| m == parts[1]) {
                if let Ok(year) = parts[2].parse::<i64>() {
                    return format!("{:04}-{:02}-{:02}", year, month_idx + 1, day);
                }
            }
        }
    }

    // Can't parse — return empty (won't filter)
    String::new()
}

const STATUS_DISPLAY_TICKS: u64 = 38; // ~3 seconds at 80ms tick
const LOAD_PULSE_TICKS: u64 = 4; // ~320ms flash
const META_DEBOUNCE_TICKS: u64 = 3; // ~240ms debounce

pub struct App {
    pub running: bool,
    pub input_mode: InputMode,
    pub active_pane: ActivePane,
    pub feed_items: Vec<PaperEntry>,
    pub feed_state: ListState,
    pub highlight_scroll: u16,
    pub article_scroll: u16,
    pub tick_count: u64,
    pub spotlight_title: String,
    pub spotlight_body: String,
    pub status_message: Option<(String, u64)>,
    pub loading: HashSet<LoadingTask>,
    pub feed_loaded_at: Option<u64>,

    pub config: Config,
    pub active_profile_key: String,
    pub profile_keys: Vec<String>,

    // Metadata cache keyed by arxiv_id
    pub meta_cache: HashMap<String, PaperMeta>,
    // HF daily paper spotlight (persists across profile switches)
    pub hf_spotlight: Option<HfSpotlight>,

    // Social feed
    pub feed_mode: FeedMode,
    pub social_items: Vec<SocialPost>,
    pub social_state: ListState,
    pub social_loaded_at: Option<u64>,

    // Filter
    pub filter_text: String,

    // Sort (paper feed only)
    pub paper_sort: PaperSort,

    // Time window (applies to both feeds)
    pub time_window: TimeWindow,

    // Max displayed items (cycleable)
    pub max_items: usize,

    // LLM providers
    pub llm_providers: Vec<Box<dyn LlmProvider>>,
    pub active_provider_idx: usize,

    // Summary modes
    pub summary_mode: SummaryMode,
    pub summary_cache: HashMap<String, String>, // "arxiv_id:mode" → text
    pub scaffold_cache: HashMap<String, String>, // arxiv_id → scaffold text
    pub full_text_cache: HashMap<String, String>, // arxiv_id → full paper text

    // Pending export (auto-triggers when full text arrives)
    pending_export_force: Option<bool>,

    // Scaffold prompt state
    pub scaffold_project_name: String,
    pub scaffold_output_dir: String,

    // Persistent scaffold path index
    pub scaffold_index: ScaffoldIndex,

    // Confirm mode state
    pub confirm_message: String,
    confirm_action: Option<Box<dyn FnOnce(&mut App) + Send>>,

    // Debounce state for meta fetches
    last_selected_arxiv_id: Option<String>,
    selection_changed_at: u64,

    net_tx: mpsc::Sender<NetworkAction>,
    net_rx: mpsc::UnboundedReceiver<NetworkEvent>,
}

impl App {
    pub fn new(
        config: Config,
        net_tx: mpsc::Sender<NetworkAction>,
        net_rx: mpsc::UnboundedReceiver<NetworkEvent>,
        llm_providers: Vec<Box<dyn LlmProvider>>,
    ) -> Self {
        let profile_keys = config.profile_keys();
        let mut active_profile_key = config.general.default_profile.clone();

        if !config.profiles.contains_key(&active_profile_key) {
            if let Some(first) = profile_keys.first() {
                active_profile_key = first.clone();
            }
        }

        // Find the active provider index by name
        let active_name = config.llm.active.clone();
        let active_provider_idx = llm_providers
            .iter()
            .position(|p| p.name() == active_name)
            .unwrap_or(0);

        Self {
            running: true,
            input_mode: InputMode::Normal,
            active_pane: ActivePane::Feed,
            feed_items: Vec::new(),
            feed_state: ListState::default(),
            highlight_scroll: 0,
            article_scroll: 0,
            tick_count: 0,
            spotlight_title: "Awaiting data\u{2026}".into(),
            spotlight_body: "Press [r] to refresh, or wait for the initial fetch.".into(),
            status_message: None,
            loading: HashSet::new(),
            feed_loaded_at: None,
            config,
            active_profile_key,
            profile_keys,
            meta_cache: HashMap::new(),
            hf_spotlight: None,
            feed_mode: FeedMode::Papers,
            social_items: Vec::new(),
            social_state: ListState::default(),
            social_loaded_at: None,
            filter_text: String::new(),
            paper_sort: PaperSort::Date,
            time_window: TimeWindow::Day,
            max_items: 50,
            llm_providers,
            active_provider_idx,
            summary_mode: SummaryMode::Off,
            summary_cache: HashMap::new(),
            scaffold_cache: HashMap::new(),
            full_text_cache: HashMap::new(),
            pending_export_force: None,
            scaffold_project_name: String::new(),
            scaffold_output_dir: "./implementations".into(),
            scaffold_index: ScaffoldIndex::load(),
            confirm_message: String::new(),
            confirm_action: None,
            last_selected_arxiv_id: None,
            selection_changed_at: 0,
            net_tx,
            net_rx,
        }
    }

    // ── Tick & Network Drain ─────────────────────────────────

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if let Some((_, set_at)) = &self.status_message {
            if self.tick_count.wrapping_sub(*set_at) > STATUS_DISPLAY_TICKS {
                self.status_message = None;
            }
        }
        self.check_meta_debounce();
    }

    fn check_meta_debounce(&mut self) {
        if self.feed_mode != FeedMode::Papers {
            return;
        }

        let current_id = self
            .selected_paper()
            .and_then(|p| p.arxiv_id.clone());

        if current_id != self.last_selected_arxiv_id {
            self.last_selected_arxiv_id = current_id;
            self.selection_changed_at = self.tick_count;
            return;
        }

        let Some(ref arxiv_id) = self.last_selected_arxiv_id else {
            return;
        };

        if self.tick_count.wrapping_sub(self.selection_changed_at) < META_DEBOUNCE_TICKS {
            return;
        }

        if self.meta_cache.contains_key(arxiv_id) {
            return;
        }

        self.meta_cache.insert(
            arxiv_id.clone(),
            PaperMeta {
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
                meta_status: MetaFetchStatus::Loading,
            },
        );

        self.loading
            .insert(LoadingTask::PaperMeta(arxiv_id.clone()));
        let _ = self
            .net_tx
            .try_send(NetworkAction::FetchPaperMeta(
                arxiv_id.clone(),
                self.config.general.enable_semantic_scholar,
            ));
    }

    pub fn drain_network_events(&mut self) {
        while let Ok(event) = self.net_rx.try_recv() {
            match event {
                NetworkEvent::FeedLoaded(papers) => {
                    self.feed_items = papers;
                    self.loading.remove(&LoadingTask::Feed);
                    self.feed_loaded_at = Some(self.tick_count);
                    if !self.feed_items.is_empty() {
                        self.feed_state.select(Some(0));
                    }
                    self.update_spotlight();
                }
                NetworkEvent::HfSpotlightLoaded(spotlight) => {
                    self.loading.remove(&LoadingTask::HfSpotlight);
                    self.spotlight_title = format!(
                        "\u{1f525} {} \u{2014} {} [\u{2191}{} upvotes]",
                        spotlight.title, spotlight.authors, spotlight.upvotes
                    );
                    self.spotlight_body = spotlight.summary.clone();
                    self.hf_spotlight = Some(spotlight);
                }
                NetworkEvent::PaperMetaLoaded { arxiv_id, meta } => {
                    self.loading
                        .remove(&LoadingTask::PaperMeta(arxiv_id.clone()));
                    self.meta_cache.insert(arxiv_id, meta);
                }
                NetworkEvent::SocialFeedLoaded(posts) => {
                    self.social_items = posts;
                    self.loading.remove(&LoadingTask::SocialFeed);
                    self.social_loaded_at = Some(self.tick_count);
                    if !self.social_items.is_empty() {
                        self.social_state.select(Some(0));
                    }
                }
                NetworkEvent::SummaryLoaded { arxiv_id, mode, text } => {
                    let key = format!("{}:{}", arxiv_id, mode);
                    self.loading.remove(&LoadingTask::LlmSummary(key.clone()));
                    self.summary_cache.insert(key, text);
                }
                NetworkEvent::ScaffoldLoaded { arxiv_id, text } => {
                    self.loading
                        .remove(&LoadingTask::LlmScaffold(arxiv_id.clone()));
                    self.scaffold_cache.insert(arxiv_id.clone(), text.clone());

                    // Write to disk
                    let dir = std::path::Path::new(&self.scaffold_output_dir)
                        .join(&self.scaffold_project_name);
                    match std::fs::create_dir_all(&dir) {
                        Ok(_) => {
                            let path = dir.join("scaffold.md");
                            match std::fs::write(&path, &text) {
                                Ok(_) => {
                                    let display_path = format!("{}", path.display());
                                    self.scaffold_index.insert(
                                        arxiv_id,
                                        display_path.clone(),
                                    );
                                    self.set_status(format!("Scaffold saved: {}", display_path));
                                }
                                Err(e) => {
                                    self.set_status(format!("Scaffold write failed: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            self.set_status(format!("Failed to create dir: {}", e));
                        }
                    }
                }
                NetworkEvent::FullTextLoaded { arxiv_id, text } => {
                    self.full_text_cache.insert(arxiv_id, text);
                    // Auto-export if pending
                    if let Some(force) = self.pending_export_force.take() {
                        self.do_export_now(force);
                    }
                }
                NetworkEvent::Error(msg) => {
                    self.set_status(format!("ERR: {}", msg));
                }
            }
        }
    }

    // ── Input: map then dispatch ─────────────────────────────

    pub fn handle_key(&mut self, key: KeyEvent) {
        if let Some(action) = self.map_key(key) {
            self.dispatch(action);
        }
    }

    fn map_key(&self, key: KeyEvent) -> Option<Action> {
        match self.input_mode {
            InputMode::Help => match key.code {
                KeyCode::Char('?') => Some(Action::ToggleHelp),
                KeyCode::Esc => Some(Action::Dismiss),
                KeyCode::Char('q') => Some(Action::Quit),
                _ => None,
            },
            InputMode::Filter => match key.code {
                KeyCode::Esc => Some(Action::Dismiss),
                KeyCode::Enter => Some(Action::ConfirmFilter),
                KeyCode::Backspace => Some(Action::FilterBackspace),
                KeyCode::Char(c) => Some(Action::FilterInput(c)),
                _ => None,
            },
            InputMode::Confirm => match key.code {
                KeyCode::Char('y') => Some(Action::ConfirmYes),
                KeyCode::Char('n') | KeyCode::Esc => Some(Action::ConfirmNo),
                _ => None,
            },
            InputMode::ScaffoldPrompt => match key.code {
                KeyCode::Esc => Some(Action::ConfirmNo),
                KeyCode::Enter => Some(Action::ScaffoldConfirm),
                KeyCode::Backspace => Some(Action::ScaffoldBackspace),
                KeyCode::Char(c) => Some(Action::ScaffoldInput(c)),
                _ => None,
            },
            InputMode::Normal => match key.code {
                KeyCode::Char('q') => Some(Action::Quit),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(Action::Quit)
                }
                KeyCode::Tab => Some(Action::NextPane),
                KeyCode::BackTab => Some(Action::PrevPane),
                KeyCode::Char('j') | KeyCode::Down => Some(Action::ScrollDown),
                KeyCode::Char('k') | KeyCode::Up => Some(Action::ScrollUp),
                KeyCode::Char('h') | KeyCode::Left => Some(Action::PrevPane),
                KeyCode::Char('l') | KeyCode::Right => Some(Action::NextPane),
                KeyCode::Char('g') => Some(Action::GoToTop),
                KeyCode::Char('G') => Some(Action::GoToBottom),
                KeyCode::Char('i') => Some(Action::SpawnImplementation),
                KeyCode::Char('o') => Some(Action::ExportToObsidian),
                KeyCode::Enter => Some(Action::OpenInBrowser),
                KeyCode::Char('p') => Some(Action::CycleProfile),
                KeyCode::Char('r') => Some(Action::RefreshFeed),
                KeyCode::Char('f') => Some(Action::ToggleFeedMode),
                KeyCode::Char('/') => Some(Action::StartFilter),
                KeyCode::Char('s') => Some(Action::CycleSort),
                KeyCode::Char('t') => Some(Action::CycleTimeWindow),
                KeyCode::Char('n') => Some(Action::CycleMaxItems),
                KeyCode::Char('m') => Some(Action::CycleSummaryMode),
                KeyCode::Char('M') => Some(Action::GenerateSummary),
                KeyCode::Char('L') => Some(Action::CycleProvider),
                KeyCode::Char('?') => Some(Action::ToggleHelp),
                KeyCode::Esc => Some(Action::Dismiss),
                _ => None,
            },
        }
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => self.running = false,
            Action::ScrollDown => self.scroll_down(),
            Action::ScrollUp => self.scroll_up(),
            Action::NextPane => self.active_pane = self.active_pane.next(),
            Action::PrevPane => self.active_pane = self.active_pane.prev(),
            Action::GoToTop => self.go_to_top(),
            Action::GoToBottom => self.go_to_bottom(),
            Action::SpawnImplementation => self.spawn_implementation(),
            Action::ExportToObsidian => self.export_to_obsidian(),
            Action::OpenInBrowser => self.open_in_browser(),
            Action::CycleProfile => self.cycle_profile(),
            Action::RefreshFeed => self.request_feed_refresh(),
            Action::ToggleHelp => {
                self.input_mode = match self.input_mode {
                    InputMode::Help => InputMode::Normal,
                    _ => InputMode::Help,
                };
            }
            Action::ToggleFeedMode => {
                self.feed_mode = match self.feed_mode {
                    FeedMode::Papers => FeedMode::Social,
                    FeedMode::Social => FeedMode::Papers,
                };
                self.article_scroll = 0;
                self.set_status(match self.feed_mode {
                    FeedMode::Papers => "Feed \u{2192} Papers",
                    FeedMode::Social => "Feed \u{2192} Social",
                });
            }
            Action::StartFilter => {
                self.input_mode = InputMode::Filter;
                self.filter_text.clear();
            }
            Action::CycleSort => {
                if self.feed_mode == FeedMode::Papers {
                    self.paper_sort = self.paper_sort.next();
                    self.set_status(format!("Sort \u{2192} {}", self.paper_sort.label()));
                }
            }
            Action::CycleTimeWindow => {
                self.time_window = self.time_window.next();
                self.set_status(format!("Time \u{2192} {}", self.time_window.label()));
            }
            Action::CycleMaxItems => {
                self.max_items = match self.max_items {
                    10 => 25,
                    25 => 50,
                    50 => 75,
                    75 => 100,
                    _ => 10,
                };
                self.set_status(format!("Max items \u{2192} {}", self.max_items));
            }
            Action::CycleSummaryMode => {
                self.summary_mode = self.summary_mode.next();
                if self.summary_mode == SummaryMode::Off {
                    self.set_status("Summary \u{2192} off");
                } else {
                    self.set_status(format!("Summary \u{2192} {}", self.summary_mode.label()));
                }
            }
            Action::GenerateSummary => {
                if !self.summary_mode.needs_llm() {
                    self.set_status("Select an LLM summary mode first (press [m] to cycle).");
                } else {
                    self.request_summary_if_needed();
                }
            }
            Action::CycleProvider => {
                if self.llm_providers.is_empty() {
                    self.set_status("No LLM providers configured.");
                } else {
                    self.active_provider_idx =
                        (self.active_provider_idx + 1) % self.llm_providers.len();
                    let p = &self.llm_providers[self.active_provider_idx];
                    self.set_status(format!("LLM \u{2192} {} ({})", p.name(), p.model()));
                }
            }
            Action::ConfirmYes => {
                if let Some(action) = self.confirm_action.take() {
                    action(self);
                }
                self.input_mode = InputMode::Normal;
                self.confirm_message.clear();
            }
            Action::ConfirmNo => {
                self.confirm_action = None;
                self.input_mode = InputMode::Normal;
                self.confirm_message.clear();
                self.set_status("Cancelled.");
            }
            Action::ScaffoldInput(c) => {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.' {
                    self.scaffold_project_name.push(c);
                }
            }
            Action::ScaffoldBackspace => {
                self.scaffold_project_name.pop();
            }
            Action::ScaffoldConfirm => {
                self.input_mode = InputMode::Normal;
                self.do_scaffold_generate();
            }
            Action::FilterInput(c) => {
                self.filter_text.push(c);
            }
            Action::FilterBackspace => {
                self.filter_text.pop();
            }
            Action::ConfirmFilter => {
                self.input_mode = InputMode::Normal;
            }
            Action::Dismiss => {
                if self.input_mode == InputMode::Filter {
                    self.filter_text.clear();
                }
                if self.input_mode != InputMode::Normal {
                    self.input_mode = InputMode::Normal;
                }
            }
        }
    }

    // ── Profile ──────────────────────────────────────────────

    pub fn active_profile(&self) -> Option<&Profile> {
        self.config.profiles.get(&self.active_profile_key)
    }

    pub fn active_profile_name(&self) -> &str {
        self.active_profile()
            .map(|p| p.name.as_str())
            .unwrap_or("unknown")
    }

    fn cycle_profile(&mut self) {
        if self.profile_keys.is_empty() {
            return;
        }
        let idx = self
            .profile_keys
            .iter()
            .position(|k| k == &self.active_profile_key)
            .map_or(0, |i| (i + 1) % self.profile_keys.len());
        self.active_profile_key = self.profile_keys[idx].clone();
        self.set_status(format!("Profile \u{2192} {}", self.active_profile_name()));
        self.request_feed_refresh();
    }

    // ── Network ──────────────────────────────────────────────

    pub fn request_feed_refresh(&mut self) {
        if let Some(profile) = self.active_profile().cloned() {
            self.loading.insert(LoadingTask::Feed);
            let _ = self.net_tx.try_send(NetworkAction::FetchFeed(profile));
        }
    }

    pub fn request_hf_spotlight(&mut self) {
        self.loading.insert(LoadingTask::HfSpotlight);
        let _ = self.net_tx.try_send(NetworkAction::FetchHfSpotlight);
    }

    pub fn request_social_refresh(&mut self) {
        let feeds = self.config.social.feeds.clone();
        let nitter = self.config.social.nitter_instance.clone();
        self.loading.insert(LoadingTask::SocialFeed);
        let _ = self
            .net_tx
            .try_send(NetworkAction::FetchSocialFeed(feeds, nitter));
    }

    // ── Dynamic Spotlight ────────────────────────────────────

    fn update_spotlight(&mut self) {
        if let Some(ref hf) = self.hf_spotlight {
            self.spotlight_title = format!(
                "\u{1f525} {} \u{2014} {} [\u{2191}{} upvotes]",
                hf.title, hf.authors, hf.upvotes
            );
            self.spotlight_body = hf.summary.clone();
            return;
        }

        if self.feed_items.is_empty() {
            return;
        }
        let keywords: Vec<String> = match self.active_profile() {
            Some(p) => p.high_weight_keywords.clone(),
            None => return,
        };

        let best = self.feed_items.iter().max_by_key(|paper| {
            let t = paper.title.to_lowercase();
            let d = paper.domain.to_lowercase();
            keywords
                .iter()
                .filter(|kw| {
                    let k = kw.to_lowercase();
                    t.contains(&k) || d.contains(&k)
                })
                .count()
        });

        if let Some(paper) = best {
            let title = format!("{} \u{2014} {}", paper.title, paper.authors);
            let body = paper
                .abstract_text
                .clone()
                .unwrap_or_else(|| {
                    format!(
                        "Top match for keywords: {}. Published {} in [{}].",
                        keywords.join(", "),
                        paper.date,
                        paper.domain,
                    )
                });
            self.spotlight_title = title;
            self.spotlight_body = body;
        }
    }

    // ── Filtering ───────────────────────────────────────────

    /// Get filtered paper indices based on current filter_text.
    pub fn filtered_paper_indices(&self) -> Vec<usize> {
        let query = self.filter_text.to_lowercase();
        let cutoff = self.time_window.cutoff_date();

        let mut indices: Vec<usize> = self.feed_items
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                // Time window filter (paper.date is "YYYY-MM-DD")
                if let Some(ref cut) = cutoff {
                    if p.date < *cut {
                        return false;
                    }
                }
                // Text filter
                if !query.is_empty() {
                    let matches = p.title.to_lowercase().contains(&query)
                        || p.authors.to_lowercase().contains(&query)
                        || p.domain.to_lowercase().contains(&query);
                    if !matches {
                        return false;
                    }
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        // Apply sort
        match self.paper_sort {
            PaperSort::Date => {} // original order from ArXiv
            PaperSort::Citations => {
                indices.sort_by(|a, b| {
                    let ca = self.feed_items[*a]
                        .arxiv_id
                        .as_ref()
                        .and_then(|id| self.meta_cache.get(id))
                        .map_or(0, |m| m.citation_count);
                    let cb = self.feed_items[*b]
                        .arxiv_id
                        .as_ref()
                        .and_then(|id| self.meta_cache.get(id))
                        .map_or(0, |m| m.citation_count);
                    cb.cmp(&ca)
                });
            }
            PaperSort::Title => {
                indices.sort_by(|a, b| self.feed_items[*a].title.cmp(&self.feed_items[*b].title));
            }
        }

        indices.truncate(self.max_items);
        indices
    }

    /// Get filtered social post indices based on current filter_text.
    pub fn filtered_social_indices(&self) -> Vec<usize> {
        let query = self.filter_text.to_lowercase();
        let cutoff = self.time_window.cutoff_date();

        let mut indices: Vec<usize> = self.social_items
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                // Time window filter — normalize published date to ISO for comparison
                if let Some(ref cut) = cutoff {
                    let iso = normalize_date_to_iso(&p.published);
                    if !iso.is_empty() && iso < *cut {
                        return false;
                    }
                }
                // Text filter
                if !query.is_empty() {
                    let matches = p.source_name.to_lowercase().contains(&query)
                        || p.title
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || p.content.to_lowercase().contains(&query);
                    if !matches {
                        return false;
                    }
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        indices.truncate(self.max_items);
        indices
    }

    // ── Queries ──────────────────────────────────────────────

    pub fn is_loading(&self) -> bool {
        !self.loading.is_empty()
    }

    pub fn has_load_pulse(&self) -> bool {
        let pulse_at = match self.feed_mode {
            FeedMode::Papers => self.feed_loaded_at,
            FeedMode::Social => self.social_loaded_at,
        };
        pulse_at
            .map(|t| self.tick_count.wrapping_sub(t) < LOAD_PULSE_TICKS)
            .unwrap_or(false)
    }

    pub fn paper_matches_keywords(&self, paper: &PaperEntry) -> bool {
        match self.active_profile() {
            Some(profile) if !profile.high_weight_keywords.is_empty() => {
                let title_lower = paper.title.to_lowercase();
                profile
                    .high_weight_keywords
                    .iter()
                    .any(|kw| title_lower.contains(&kw.to_lowercase()))
            }
            _ => false,
        }
    }

    pub fn selected_paper(&self) -> Option<&PaperEntry> {
        let indices = self.filtered_paper_indices();
        self.feed_state
            .selected()
            .and_then(|i| indices.get(i))
            .and_then(|&orig| self.feed_items.get(orig))
    }

    pub fn selected_social_post(&self) -> Option<&SocialPost> {
        let indices = self.filtered_social_indices();
        self.social_state
            .selected()
            .and_then(|i| indices.get(i))
            .and_then(|&orig| self.social_items.get(orig))
    }

    pub fn selected_paper_meta(&self) -> Option<&PaperMeta> {
        self.selected_paper()
            .and_then(|p| p.arxiv_id.as_ref())
            .and_then(|id| self.meta_cache.get(id))
    }

    /// (1-indexed position, total count)
    pub fn feed_position(&self) -> (usize, usize) {
        match self.feed_mode {
            FeedMode::Papers => {
                let total = self.filtered_paper_indices().len();
                let sel = self.feed_state.selected().map_or(0, |i| i + 1);
                (sel, total)
            }
            FeedMode::Social => {
                let total = self.filtered_social_indices().len();
                let sel = self.social_state.selected().map_or(0, |i| i + 1);
                (sel, total)
            }
        }
    }

    // ── Scroll ───────────────────────────────────────────────

    fn scroll_down(&mut self) {
        match self.active_pane {
            ActivePane::Feed => match self.feed_mode {
                FeedMode::Papers => {
                    let len = self.filtered_paper_indices().len();
                    if len == 0 {
                        return;
                    }
                    let i = self
                        .feed_state
                        .selected()
                        .map_or(0, |i| if i >= len - 1 { 0 } else { i + 1 });
                    self.feed_state.select(Some(i));
                }
                FeedMode::Social => {
                    let len = self.filtered_social_indices().len();
                    if len == 0 {
                        return;
                    }
                    let i = self
                        .social_state
                        .selected()
                        .map_or(0, |i| if i >= len - 1 { 0 } else { i + 1 });
                    self.social_state.select(Some(i));
                }
            },
            ActivePane::Highlight => {
                self.highlight_scroll = self.highlight_scroll.saturating_add(1);
            }
            ActivePane::Article => {
                self.article_scroll = self.article_scroll.saturating_add(1);
            }
        }
    }

    fn scroll_up(&mut self) {
        match self.active_pane {
            ActivePane::Feed => match self.feed_mode {
                FeedMode::Papers => {
                    let len = self.filtered_paper_indices().len();
                    if len == 0 {
                        return;
                    }
                    let i = self
                        .feed_state
                        .selected()
                        .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
                    self.feed_state.select(Some(i));
                }
                FeedMode::Social => {
                    let len = self.filtered_social_indices().len();
                    if len == 0 {
                        return;
                    }
                    let i = self
                        .social_state
                        .selected()
                        .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
                    self.social_state.select(Some(i));
                }
            },
            ActivePane::Highlight => {
                self.highlight_scroll = self.highlight_scroll.saturating_sub(1);
            }
            ActivePane::Article => {
                self.article_scroll = self.article_scroll.saturating_sub(1);
            }
        }
    }

    fn go_to_top(&mut self) {
        match self.active_pane {
            ActivePane::Feed => match self.feed_mode {
                FeedMode::Papers if !self.filtered_paper_indices().is_empty() => {
                    self.feed_state.select(Some(0));
                }
                FeedMode::Social if !self.filtered_social_indices().is_empty() => {
                    self.social_state.select(Some(0));
                }
                _ => {}
            },
            ActivePane::Highlight => self.highlight_scroll = 0,
            ActivePane::Article => self.article_scroll = 0,
        }
    }

    fn go_to_bottom(&mut self) {
        match self.active_pane {
            ActivePane::Feed => match self.feed_mode {
                FeedMode::Papers => {
                    let len = self.filtered_paper_indices().len();
                    if len > 0 {
                        self.feed_state.select(Some(len - 1));
                    }
                }
                FeedMode::Social => {
                    let len = self.filtered_social_indices().len();
                    if len > 0 {
                        self.social_state.select(Some(len - 1));
                    }
                }
            },
            ActivePane::Highlight => self.highlight_scroll = u16::MAX / 2,
            ActivePane::Article => self.article_scroll = u16::MAX / 2,
        }
    }

    // ── Actions ──────────────────────────────────────────────

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), self.tick_count));
    }

    fn spawn_implementation(&mut self) {
        if self.llm_providers.is_empty() {
            self.set_status("No LLM provider configured. Set API key in config.toml");
            return;
        }
        let Some(paper) = self.selected_paper() else {
            self.set_status("No paper selected.");
            return;
        };
        let arxiv_id = paper.arxiv_id.clone().unwrap_or_default();
        if arxiv_id.is_empty() {
            self.set_status("Paper has no arxiv_id.");
            return;
        }

        // Default project name from paper title
        let default_name: String = paper
            .title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
        self.scaffold_project_name = if default_name.len() > 40 {
            default_name[..40].trim_end_matches('-').to_string()
        } else {
            default_name
        };

        // Check if scaffold already exists
        if let Some(path) = self.scaffold_index.get(&arxiv_id).map(|s| s.to_string()) {
            self.confirm_message = format!(
                "Scaffold exists at: {}. Regenerate?",
                path
            );
            self.input_mode = InputMode::Confirm;
            self.confirm_action = Some(Box::new(move |app: &mut App| {
                app.input_mode = InputMode::ScaffoldPrompt;
            }));
            return;
        }

        self.input_mode = InputMode::ScaffoldPrompt;
    }

    fn do_scaffold_generate(&mut self) {
        let Some(paper) = self.selected_paper() else {
            return;
        };
        let arxiv_id = paper.arxiv_id.clone().unwrap_or_default();
        let title = paper.title.clone();
        let abstract_text = paper.abstract_text.clone().unwrap_or_default();

        self.loading
            .insert(LoadingTask::LlmScaffold(arxiv_id.clone()));
        let _ = self.net_tx.try_send(NetworkAction::GenerateScaffold {
            arxiv_id,
            title,
            abstract_text,
            provider_idx: self.active_provider_idx,
        });
        self.set_status(format!(
            "Generating scaffold for {}/{}...",
            self.scaffold_output_dir, self.scaffold_project_name
        ));
    }

    fn export_to_obsidian(&mut self) {
        let Some(paper) = self.selected_paper().cloned() else {
            self.set_status("No paper selected.");
            return;
        };
        let arxiv_id = paper.arxiv_id.clone().unwrap_or_default();
        if arxiv_id.is_empty() {
            self.set_status("Paper has no arxiv_id.");
            return;
        }

        // Check for duplicate BEFORE downloading
        let exists = crate::obsidian::paper_exists(&arxiv_id, &self.config.obsidian);
        if exists {
            // Ask confirmation, then proceed with full pipeline on yes
            self.confirm_message = "Paper already exported. [y] replace  [n] cancel".into();
            self.input_mode = InputMode::Confirm;
            let id = arxiv_id.clone();
            self.confirm_action = Some(Box::new(move |app: &mut App| {
                app.do_export_pipeline(&id, true);
            }));
            return;
        }

        self.do_export_pipeline(&arxiv_id, false);
    }

    /// Single-step export: fetch full text if needed, then export immediately when it arrives.
    fn do_export_pipeline(&mut self, arxiv_id: &str, force: bool) {
        // If full text already cached, export now
        if self.full_text_cache.contains_key(arxiv_id) {
            self.do_export_now(force);
            return;
        }

        // Stash the force flag so we can export when text arrives
        self.pending_export_force = Some(force);
        let _ = self
            .net_tx
            .try_send(NetworkAction::FetchFullText(arxiv_id.to_string()));
        self.set_status("Downloading full paper... will export automatically.");
    }

    fn do_export_now(&mut self, force: bool) {
        let Some(paper) = self.selected_paper().cloned() else {
            return;
        };
        let meta = self.selected_paper_meta().cloned();
        let summary_key = paper
            .arxiv_id
            .as_deref()
            .map(|id| format!("{}:{}", id, self.summary_mode.api_key()));
        let summary = summary_key
            .as_ref()
            .and_then(|k| self.summary_cache.get(k))
            .map(|s| s.as_str());
        let scaffold = paper
            .arxiv_id
            .as_deref()
            .and_then(|id| self.scaffold_cache.get(id))
            .map(|s| s.as_str());
        let full_text = paper
            .arxiv_id
            .as_deref()
            .and_then(|id| self.full_text_cache.get(id))
            .map(|s| s.as_str());

        match crate::obsidian::export_paper(
            &paper,
            meta.as_ref(),
            summary,
            scaffold,
            full_text,
            &self.config.obsidian,
            force,
        ) {
            Ok(crate::obsidian::ExportResult::Created(path))
            | Ok(crate::obsidian::ExportResult::Updated(path)) => {
                self.set_status(format!(
                    "Exported: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Ok(crate::obsidian::ExportResult::AlreadyExists(_)) => {
                // Shouldn't happen since we check beforehand, but handle gracefully
                self.set_status("Already exported (use [o] again to replace).");
            }
            Err(e) => {
                self.set_status(format!("ERR: {}", e));
            }
        }
    }

    fn request_summary_if_needed(&mut self) {
        if !self.summary_mode.needs_llm() {
            return;
        }
        if self.llm_providers.is_empty() {
            self.set_status("No LLM provider configured.");
            return;
        }
        let Some(paper) = self.selected_paper() else {
            return;
        };
        let arxiv_id = match paper.arxiv_id.as_ref() {
            Some(id) => id.clone(),
            None => return,
        };
        let mode_key = self.summary_mode.api_key().to_string();
        let cache_key = format!("{}:{}", arxiv_id, mode_key);

        if self.summary_cache.contains_key(&cache_key) {
            return; // already cached
        }

        let abstract_text = paper.abstract_text.clone().unwrap_or_default();
        self.loading
            .insert(LoadingTask::LlmSummary(cache_key));
        let _ = self.net_tx.try_send(NetworkAction::Summarize {
            arxiv_id,
            mode: mode_key,
            abstract_text,
            provider_idx: self.active_provider_idx,
        });
    }

    fn open_in_browser(&mut self) {
        let url = match self.active_pane {
            ActivePane::Highlight => {
                // Open spotlight paper on HF
                self.hf_spotlight.as_ref()
                    .filter(|hf| !hf.arxiv_id.is_empty())
                    .map(|hf| format!("https://huggingface.co/papers/{}", hf.arxiv_id))
            }
            _ => match self.feed_mode {
                FeedMode::Papers => {
                    self.selected_paper().and_then(|p| {
                        p.arxiv_id.as_ref().map(|id| format!("https://arxiv.org/abs/{}", id))
                    })
                }
                FeedMode::Social => {
                    self.selected_social_post()
                        .map(|p| p.url.clone())
                        .filter(|u| !u.is_empty())
                }
            }
        };

        match url {
            Some(u) => {
                let display = if u.len() > 60 {
                    format!("{}...", &u[..57])
                } else {
                    u.clone()
                };
                self.set_status(format!("Opening: {}", display));
                // macOS: open, Linux: xdg-open
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(&u).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(&u).spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd").args(["/C", "start", &u]).spawn();
            }
            None => {
                self.set_status("No URL available.");
            }
        }
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use serde::{Deserialize, Serialize};

// ── Top-level Config ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub profiles: HashMap<String, Profile>,
    pub llm: LlmConfig,
    pub obsidian: ObsidianConfig,
    pub social: SocialConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub default_profile: String,
    pub tick_rate_ms: u64,
    pub max_feed_items: usize,
    #[serde(default)]
    pub enable_semantic_scholar: bool,
    #[serde(default = "default_implementations_dir")]
    pub implementations_dir: String,
}

fn default_implementations_dir() -> String {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".tensor_term")
        .join("implementations")
        .to_string_lossy()
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub arxiv_categories: Vec<String>,
    pub high_weight_keywords: Vec<String>,
    #[serde(default = "default_feed_sources")]
    pub feed_sources: Vec<String>,
}

fn default_feed_sources() -> Vec<String> {
    vec!["arxiv".into()]
}

// ── LLM Provider Config ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub active: String,
    pub anthropic: Option<AnthropicLlmConfig>,
    pub openai: Option<OpenAiLlmConfig>,
    #[serde(default)]
    pub openai_compatible: Vec<OpenAiCompatEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicLlmConfig {
    pub api_key: Option<String>,
    #[serde(default = "default_anthropic_model")]
    pub model: String,
}

fn default_anthropic_model() -> String {
    "claude-sonnet-4-20250514".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiLlmConfig {
    pub api_key: Option<String>,
    #[serde(default = "default_openai_model")]
    pub model: String,
}

fn default_openai_model() -> String {
    "gpt-4o".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiCompatEntry {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

// ── Other Config Sections ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObsidianConfig {
    pub vault_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SocialConfig {
    pub nitter_instance: String,
    pub feeds: Vec<SocialFeedConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialFeedConfig {
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

// ── Config Implementation ────────────────────────────────────

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut config: Config = toml::from_str(&content)?;
            config.resolve_env_vars();
            Ok(config)
        } else {
            Self::write_default_template(&path)?;
            let mut config = Self::default();
            config.resolve_env_vars();
            Ok(config)
        }
    }

    pub fn config_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        base.join("tensor_term").join("config.toml")
    }

    fn write_default_template(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, DEFAULT_CONFIG_TEMPLATE)?;
        Ok(())
    }

    fn resolve_env_vars(&mut self) {
        // Anthropic key from env
        if let Some(ref mut cfg) = self.llm.anthropic {
            if cfg.api_key.is_none() {
                cfg.api_key = std::env::var("ANTHROPIC_API_KEY").ok();
            }
        } else if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            self.llm.anthropic = Some(AnthropicLlmConfig {
                api_key: Some(key),
                model: default_anthropic_model(),
            });
        }

        // OpenAI key from env
        if let Some(ref mut cfg) = self.llm.openai {
            if cfg.api_key.is_none() {
                cfg.api_key = std::env::var("OPENAI_API_KEY").ok();
            }
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            self.llm.openai = Some(OpenAiLlmConfig {
                api_key: Some(key),
                model: default_openai_model(),
            });
        }
    }

    pub fn profile_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.profiles.keys().cloned().collect();
        keys.sort();
        keys
    }
}

// ── Defaults ─────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "generative".into(),
            Profile {
                name: "Generative Models".into(),
                arxiv_categories: vec!["cs.CL".into(), "cs.LG".into()],
                high_weight_keywords: vec![
                    "Generative Flows".into(),
                    "TimesFM".into(),
                    "LLaMA".into(),
                ],
                feed_sources: vec!["arxiv".into()],
            },
        );
        profiles.insert(
            "rl_agents".into(),
            Profile {
                name: "RL Agents".into(),
                arxiv_categories: vec!["cs.AI".into()],
                high_weight_keywords: vec![
                    "DDPG".into(),
                    "PPO".into(),
                    "TD3".into(),
                    "Multi-Agent".into(),
                ],
                feed_sources: vec!["arxiv".into()],
            },
        );

        Self {
            general: GeneralConfig::default(),
            profiles,
            llm: LlmConfig::default(),
            obsidian: ObsidianConfig::default(),
            social: SocialConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_profile: "generative".into(),
            tick_rate_ms: 80,
            max_feed_items: 50,
            enable_semantic_scholar: false,
            implementations_dir: default_implementations_dir(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            active: "anthropic".into(),
            anthropic: None,
            openai: None,
            openai_compatible: Vec::new(),
        }
    }
}

impl Default for ObsidianConfig {
    fn default() -> Self {
        Self {
            vault_path: String::new(),
        }
    }
}

impl Default for SocialConfig {
    fn default() -> Self {
        Self {
            nitter_instance: "https://nitter.net".into(),
            feeds: vec![
                SocialFeedConfig { name: "Andrej Karpathy".into(), source: "twitter:karpathy".into(), keywords: vec![] },
                SocialFeedConfig { name: "Yann LeCun".into(), source: "twitter:ylecun".into(), keywords: vec![] },
                SocialFeedConfig { name: "Sam Altman".into(), source: "twitter:sama".into(), keywords: vec![] },
                SocialFeedConfig { name: "Dario Amodei".into(), source: "twitter:DarioAmodei".into(), keywords: vec![] },
                SocialFeedConfig { name: "Ilya Sutskever".into(), source: "twitter:ilyasut".into(), keywords: vec![] },
                SocialFeedConfig { name: "Dwarkesh Patel".into(), source: "rss:https://www.dwarkeshpatel.com/feed".into(), keywords: vec![] },
                SocialFeedConfig {
                    name: "Elon Musk".into(),
                    source: "twitter:elonmusk".into(),
                    keywords: vec!["AI".into(), "xAI".into(), "Grok".into(), "compute".into(), "neural".into(), "AGI".into()],
                },
            ],
        }
    }
}

// ── Default config template ──────────────────────────────────

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# ╔══════════════════════════════════════════════╗
# ║  TensorTerm — Configuration                 ║
# ╚══════════════════════════════════════════════╝

[general]
default_profile = "generative"
tick_rate_ms = 80
max_feed_items = 50
# enable_semantic_scholar = true  # off by default (rate limited)
# implementations_dir = "~/.tensor_term/implementations"  # where scaffolds are saved

# ── Research Profiles ─────────────────────────────

[profiles.generative]
name = "Generative Models"
arxiv_categories = ["cs.CL", "cs.LG"]
high_weight_keywords = ["Generative Flows", "TimesFM", "LLaMA"]
feed_sources = ["arxiv"]

[profiles.rl_agents]
name = "RL Agents"
arxiv_categories = ["cs.AI"]
high_weight_keywords = ["DDPG", "PPO", "TD3", "Multi-Agent"]
feed_sources = ["arxiv"]

# ── LLM Providers ────────────────────────────────
#
# Keys can also be set via environment variables:
#   ANTHROPIC_API_KEY, OPENAI_API_KEY

[llm]
active = "anthropic"

[llm.anthropic]
model = "claude-sonnet-4-20250514"
# api_key = "sk-ant-..."

[llm.openai]
model = "gpt-4o"
# api_key = "sk-..."

# Add any OpenAI-compatible endpoints:
# [[llm.openai_compatible]]
# name = "ollama"
# base_url = "http://localhost:11434/v1"
# model = "llama3"

# [[llm.openai_compatible]]
# name = "openrouter"
# base_url = "https://openrouter.ai/api/v1"
# api_key = "sk-or-..."
# model = "anthropic/claude-3.5-sonnet"

# ── Obsidian Integration ─────────────────────────

[obsidian]
vault_path = ""
# Set to your Obsidian vault path, e.g.:
# vault_path = "/Users/you/ObsidianVault"

# ── Social / Thought Leader Feed ───────────────

[social]
nitter_instance = "https://nitter.net"

[[social.feeds]]
name = "Andrej Karpathy"
source = "twitter:karpathy"

[[social.feeds]]
name = "Yann LeCun"
source = "twitter:ylecun"

[[social.feeds]]
name = "Sam Altman"
source = "twitter:sama"

[[social.feeds]]
name = "Dario Amodei"
source = "twitter:DarioAmodei"

[[social.feeds]]
name = "Ilya Sutskever"
source = "twitter:ilyasut"

[[social.feeds]]
name = "Dwarkesh Patel"
source = "rss:https://www.dwarkeshpatel.com/feed"

[[social.feeds]]
name = "Elon Musk"
source = "twitter:elonmusk"
keywords = ["AI", "xAI", "Grok", "compute", "neural", "AGI"]
"#;

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo run            # Build and run the TUI dashboard
cargo check          # Type-check without building
cargo build --release  # Optimized release build
```

No test suite exists yet. Debug logs write to `~/.config/tensor_term/debug.log` — use `tail -f` in a second terminal while the TUI runs.

## Architecture

**TensorTerm** is a cyberpunk terminal dashboard for ML research paper tracking and thought leader social feeds, built with Rust + ratatui.

### Event Loop (main.rs → event.rs → app.rs)

Three concurrent systems communicate via channels:

1. **OS thread** (event.rs): Polls crossterm keyboard/mouse events via `std::sync::mpsc`, emits `Event::Tick` at 80ms intervals
2. **Tokio async worker** (network.rs): Receives `NetworkAction` via `tokio::sync::mpsc` (bounded, 32), sends back `NetworkEvent` via unbounded channel. Each action is **inner-spawned** (`tokio::spawn`) so slow fetches don't block others
3. **Main thread**: Renders UI, drains network events non-blocking on each tick, dispatches key actions

### Action Dispatch Pattern (app.rs)

All input flows through: `handle_key()` → `map_key()` (pure, returns `Option<Action>` based on `InputMode`) → `dispatch()` (mutates state). This separation keeps input mapping testable and decoupled from state mutation.

**InputMode** gates key interpretation: `Normal` | `Help` | `Filter` | `Confirm` | `ScaffoldPrompt`

### Data Providers (src/providers/)

Each provider is an async function taking `&reqwest::Client` and returning a Result. Two shared clients exist:
- `build_client()`: 15s timeout for API calls
- `build_llm_client()`: 120s timeout for LLM generation

| Provider | API | Returns |
|----------|-----|---------|
| `arxiv.rs` | ArXiv Atom XML | `Vec<PaperEntry>` — 50 papers sorted by submission date |
| `huggingface.rs` | HF daily papers | `HfSpotlight` — highest upvoted paper |
| `hf_papers.rs` | HF `/api/papers/{id}` | GitHub repo, stars, upvotes, AI summary, keywords |
| `semantic_scholar.rs` | S2 Graph API | Citation counts, top citing papers (opt-in, rate limited) |
| `social.rs` | RSS/Atom + Nitter | `Vec<SocialPost>` — aggregated social feed |
| `arxiv_html.rs` | ArXiv HTML rendering | Full paper text extraction |

### LLM Provider Trait (src/llm/)

```rust
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn chat(&self, client, messages, max_tokens) -> Pin<Box<dyn Future<...>>>;
}
```

No `async-trait` dependency — uses manual `Pin<Box<dyn Future>>`. Pre-built implementations: `AnthropicProvider` (Messages API) and `OpenAiCompatProvider` (works with OpenAI, Ollama, OpenRouter, any OpenAI-compatible endpoint).

Providers are built from config at startup, stored as `Vec<Box<dyn LlmProvider>>`, shared with the network worker via `Arc`. The app holds a separate copy for UI display (name/model).

### UI Widget System (src/ui/)

Layout: Header (5 rows) | Content (left 38% + right 62%) | Status bar (1 row). Left column splits into Feed (62%) and Spotlight (38%).

Key patterns:
- `pane_block()` helper: Generates styled borders with active/inactive/pulse states
- `SPINNER`: 10-frame braille animation chars, indexed by `tick_count`
- All widget data uses owned `String` to avoid borrow conflicts with `ListState`
- Modals (help, confirm, scaffold prompt) render last as overlays using `Clear` widget

### Filtering & Display

`filtered_paper_indices()` and `filtered_social_indices()` compute display-ready index vectors each render, applying:
1. Time window filter (24h/7d/30d/all)
2. Text filter (title/authors/domain)
3. Sort order (date/citations/title, papers only)
4. Max items truncation

Selection methods (`selected_paper()`, etc.) map through these filtered indices — the `ListState` position refers to the filtered list, not the raw data vector.

### Meta Debounce

Paper metadata (S2 + HF) is fetched 3 ticks (~240ms) after selection stabilizes. Prevents API thrashing during rapid scrolling. Results cached in `meta_cache: HashMap<String, PaperMeta>` keyed by arxiv_id.

## Key Conventions

- **Status messages**: `set_status(msg)` stores `(String, tick_count)`, auto-clears after 38 ticks (~3s)
- **Loading state**: `LoadingTask` enum in a `HashSet` — granular per-action tracking
- **Load pulse**: 4-tick cyan border flash when data arrives (`has_load_pulse()`)
- **Graceful degradation**: S2 404 → `s2_found: false`; HF 404 → empty meta; ArXiv fail → mock data fallback
- **Error flow**: Network errors send `NetworkEvent::Error(String)` → displayed in status bar, never panics
- **Nitter URL rewrite**: Twitter post URLs rewritten from nitter hostname to `x.com`
- **Date normalization**: `normalize_date_to_iso()` handles both ISO-8601 and RFC 2822 date formats for time window filtering

## Persistence

| File | Location | Purpose |
|------|----------|---------|
| Config | `~/.config/tensor_term/config.toml` | Auto-created from template on first run |
| Debug log | `~/.config/tensor_term/debug.log` | Append-only, use `debug_log!()` macro |
| Scaffold index | `~/.config/tensor_term/scaffold_index.json` | Maps arxiv_id → scaffold file path |
| Obsidian export | `{vault_path}/tensor_term_kb/{arxiv_id}_{slug}.md` | Full paper markdown with frontmatter |

## Config: Environment Variable Fallback

`ANTHROPIC_API_KEY` and `OPENAI_API_KEY` env vars auto-populate the LLM config if not set in TOML. Semantic Scholar is disabled by default (`enable_semantic_scholar = false`).

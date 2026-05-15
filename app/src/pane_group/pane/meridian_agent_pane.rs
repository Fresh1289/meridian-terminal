//! `MeridianAgent` pane — chat-style display of an existing Claude Code session JSONL.
//!
//! Phase 3b-A shipped a read-only debug view of the JSONL. Phase 3b-B + 3b-E (this round)
//! turn that into something that feels like a real chat surface:
//!
//! - **Live refresh**: a `notify-debouncer-full` watcher on the JSONL re-reads the
//!   transcript whenever Manager (or any `claude` process) appends new turns. Watcher
//!   events feed an `mpsc` channel that's drained by `ctx.spawn_stream_local`.
//! - **Markdown** rendering for assistant text via `markdown_parser::parse_markdown`
//!   into a `FormattedTextElement` (headings, bold, italic, lists, inline code,
//!   code blocks rendered in monospace).
//! - **Selection + copy**: each text element is wrapped by `SelectableArea` for
//!   drag-select; selected text is mirrored to a shared `Arc<RwLock<Option<String>>>`
//!   that the right-click "Copy" handler reads when writing to the clipboard.
//! - **Visual differentiation** by turn-block kind: text vs `tool_use` (compact icon
//!   line) vs `tool_result` (indented, error-colored on `is_error`).
//! - **Fold long tool_results** at 10 lines / 500 chars with a static "(truncated, N lines)"
//!   marker. (Click-to-expand state plumbing is wired but the toggle UI is deferred.)
//!
//! Architecture: still bypasses the `AIConversation` / `BlocklistAIHistoryModel` path
//! (server-synced via gRPC). This is pure local rendering — the JSONL on disk is the
//! source of truth, and the watcher loops the rendering on every append.
//!
//! Out of scope this round: input from pane (3b-C), launcher (3b-D), persistence,
//! LaTeX math, streaming intra-turn rendering, syntax highlighting in code blocks
//! (FormattedTextElement renders code blocks in monospace; a future round can
//! integrate `syntax_tree` for token-level coloring).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use markdown_parser::{parse_markdown, FormattedText, FormattedTextFragment, FormattedTextLine};
use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, Debouncer, RecommendedCache};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp_core::ui::appearance::Appearance;
use warpui::clipboard::ClipboardContent;
use warpui::elements::{
    ClippedScrollStateHandle, ClippedScrollable, Container, CrossAxisAlignment, Fill, Flex,
    FormattedTextElement, HeadingFontSizeMultipliers, MainAxisSize, ParentElement, SavePosition,
    ScrollTarget, ScrollToPositionMode, ScrollbarWidth, SelectableArea, SelectionHandle, Text,
};
use warpui::keymap::EditableBinding;
use warpui::r#async::SpawnedLocalStream;
use warpui::{
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use super::view::{HeaderContent, HeaderRenderContext};
use super::{view::PaneView, DetachType, PaneConfiguration, PaneContent, PaneGroup, PaneId};
use super::{ShareableLink, ShareableLinkError};
use crate::ai::agent_sdk::driver::harness::claude_transcript::read_jsonl;
use crate::app_state::LeafContents;
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::pane::BackingView;
use crate::pane_group::PaneEvent;

/// Register keybindings scoped to the `MeridianAgentView` context. Called from
/// `pane/mod.rs::init`. Currently binds Cmd+C / Ctrl+C to copy the active text
/// selection to the clipboard.
pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_editable_bindings([EditableBinding::new(
        "meridian_agent:copy_selection",
        "Meridian Agent: Copy selected text",
        MeridianAgentAction::CopySelection,
    )
    .with_context_predicate(id!("MeridianAgentView"))
    .with_mac_key_binding("cmd-c")
    .with_linux_or_windows_key_binding("ctrl-c")]);
}

const SCROLL_BOTTOM_POSITION_ID: &str = "meridian-agent-bottom";
const TOOL_RESULT_FOLD_LINE_LIMIT: usize = 10;
const TOOL_RESULT_FOLD_CHAR_LIMIT: usize = 500;
const TOOL_RESULT_FOLD_PREVIEW_LINES: usize = 3;
const TOOL_USE_INPUT_PREVIEW_CHARS: usize = 80;
const FILE_WATCH_DEBOUNCE_MS: u64 = 150;

/// One block within a turn. Mirrors the relevant `message.content[*].type`
/// values from a Claude Code JSONL entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnBlock {
    /// Markdown-bearing text from the assistant or user.
    Text(String),
    /// An assistant tool invocation. `input_summary` is a one-line preview pulled
    /// from the most informative key in `input` (command/file_path/etc.) or the
    /// compact JSON dump as a fallback.
    ToolUse { name: String, input_summary: String },
    /// A tool execution result. Sent back to the model in a `user`-typed JSONL
    /// entry; we surface it inline with the assistant's `tool_use` for display.
    ToolResult { content: String, is_error: bool },
}

impl TurnBlock {
    /// Plain-text rendering used for tests, log output, and selection fallback.
    pub fn as_plain_text(&self) -> String {
        match self {
            TurnBlock::Text(text) => text.clone(),
            TurnBlock::ToolUse {
                name,
                input_summary,
            } => {
                if input_summary.is_empty() {
                    format!("[tool: {name}]")
                } else {
                    format!("[tool: {name}] {input_summary}")
                }
            }
            TurnBlock::ToolResult { content, is_error } => {
                if *is_error {
                    format!("[tool result, error] {content}")
                } else {
                    format!("[tool result] {content}")
                }
            }
        }
    }
}

/// One turn (single JSONL entry) extracted from a Claude Code session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeridianTurn {
    /// Role label as it appears in the JSONL: `"user"`, `"assistant"`, or `"system"`.
    pub role: String,
    /// ISO-8601 timestamp string from the JSONL `timestamp` field, if present.
    pub timestamp: Option<String>,
    /// Ordered content blocks. Text blocks are markdown-bearing; tool blocks
    /// are surfaced separately so the renderer can style them distinctly.
    pub blocks: Vec<TurnBlock>,
}

impl MeridianTurn {
    /// Concatenated plain-text body — useful in tests and for quick logging.
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(TurnBlock::as_plain_text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Lightweight transcript representation: a flat list of turns in source order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeridianAgentTranscript {
    pub turns: Vec<MeridianTurn>,
}

impl MeridianAgentTranscript {
    /// Read the JSONL at `path` and convert recognized entries to displayable turns.
    /// Skips bookkeeping entries (`file-history-snapshot`, etc.) and returns an empty
    /// transcript if the file does not exist.
    pub fn load_from_jsonl(path: &Path) -> anyhow::Result<Self> {
        let entries = read_jsonl(path)?;
        let mut turns = Vec::with_capacity(entries.len());
        for entry in &entries {
            let Some(ty) = entry.get("type").and_then(|v| v.as_str()) else {
                continue;
            };
            let role = match ty {
                "user" | "assistant" | "system" => ty.to_owned(),
                _ => continue,
            };
            let blocks = extract_blocks(entry);
            if blocks.is_empty() {
                continue;
            }
            let timestamp = entry
                .get("timestamp")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            turns.push(MeridianTurn {
                role,
                timestamp,
                blocks,
            });
        }
        Ok(Self { turns })
    }
}

fn extract_blocks(entry: &serde_json::Value) -> Vec<TurnBlock> {
    let content = entry
        .get("message")
        .and_then(|m| m.get("content"))
        .or_else(|| entry.get("content"));
    let Some(content) = content else {
        return Vec::new();
    };
    match content {
        serde_json::Value::String(s) if !s.is_empty() => vec![TurnBlock::Text(s.clone())],
        serde_json::Value::String(_) => Vec::new(),
        serde_json::Value::Array(blocks) => {
            let mut out = Vec::with_capacity(blocks.len());
            for block in blocks {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                out.push(TurnBlock::Text(text.to_owned()));
                            }
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_owned();
                        let input_summary = summarize_tool_input(block.get("input"));
                        out.push(TurnBlock::ToolUse {
                            name,
                            input_summary,
                        });
                    }
                    "tool_result" => {
                        let content = block
                            .get("content")
                            .map(render_tool_result_content)
                            .unwrap_or_default();
                        let is_error = block
                            .get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        out.push(TurnBlock::ToolResult { content, is_error });
                    }
                    _ => {}
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn summarize_tool_input(input: Option<&serde_json::Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    // Prefer well-known "headline" fields — this is what a human reader scans for.
    for key in [
        "command",
        "file_path",
        "path",
        "pattern",
        "description",
        "url",
        "query",
    ] {
        if let Some(v) = input.get(key).and_then(|v| v.as_str()) {
            return truncate_for_preview(v, TOOL_USE_INPUT_PREVIEW_CHARS);
        }
    }
    let json = serde_json::to_string(input).unwrap_or_default();
    truncate_for_preview(&json, TOOL_USE_INPUT_PREVIEW_CHARS)
}

fn truncate_for_preview(s: &str, max_chars: usize) -> String {
    let mut s = s.replace('\n', " ");
    let count = s.chars().count();
    if count > max_chars {
        let cut = s
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        s.truncate(cut);
        s.push('…');
    }
    s
}

fn render_tool_result_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                    b.get("text").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

/// Returns `(preview_text, total_lines, was_truncated)`. Used to fold long tool results.
fn fold_tool_result(content: &str) -> (String, usize, bool) {
    let total_lines = content.lines().count().max(1);
    let too_long = total_lines > TOOL_RESULT_FOLD_LINE_LIMIT
        || content.chars().count() > TOOL_RESULT_FOLD_CHAR_LIMIT;
    if !too_long {
        return (content.to_owned(), total_lines, false);
    }
    let preview = content
        .lines()
        .take(TOOL_RESULT_FOLD_PREVIEW_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    (preview, total_lines, true)
}

/// In-memory load state for the pane.
#[derive(Debug, Clone)]
enum LoadState {
    Loaded(MeridianAgentTranscript),
    Error(String),
}

/// A debouncer with the recommended OS watcher + recommended cache. Held by the view to
/// keep the watcher OS thread alive — drops automatically when the view drops, which
/// tears down the watcher.
type FileWatcher = Debouncer<RecommendedWatcher, RecommendedCache>;

/// View that owns the loaded transcript, the file watcher, and the selection state.
pub struct MeridianAgentView {
    role: String,
    session_uuid: Uuid,
    jsonl_path: PathBuf,
    state: LoadState,
    pane_configuration: ModelHandle<PaneConfiguration>,
    focus_handle: Option<PaneFocusHandle>,
    scroll_state: ClippedScrollStateHandle,
    selection_handle: SelectionHandle,
    /// Mirror of the SelectableArea's selection text, populated by the selection
    /// callback on every update. Read by `copy_selection_to_clipboard`.
    selected_text: Arc<RwLock<Option<String>>>,
    /// Tool-result blocks currently expanded by the user, indexed by `(turn_idx, block_idx)`.
    /// Plumbing exists; click-to-expand wiring is deferred (see file doc).
    expanded_tool_results: HashSet<(usize, usize)>,
    /// File watcher; held to keep the watcher OS thread alive. Dropped on view drop.
    _watcher: Option<FileWatcher>,
    /// Spawned stream pump that drains the watcher → reload signal channel; held to
    /// keep the spawn alive. Dropped on view drop, which cancels the pump.
    _watch_stream: Option<SpawnedLocalStream>,
}

impl MeridianAgentView {
    pub fn new(
        role: String,
        session_uuid: Uuid,
        jsonl_path: PathBuf,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let pane_configuration =
            ctx.add_model(|_| PaneConfiguration::new(format!("Agent: {role}")));
        let scroll_state = ClippedScrollStateHandle::new();
        let selection_handle = SelectionHandle::default();
        let selected_text: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        let state = match MeridianAgentTranscript::load_from_jsonl(&jsonl_path) {
            Ok(transcript) => LoadState::Loaded(transcript),
            Err(err) => LoadState::Error(format!("{err:#}")),
        };
        scroll_state.scroll_to_position(ScrollTarget {
            position_id: SCROLL_BOTTOM_POSITION_ID.to_string(),
            mode: ScrollToPositionMode::FullyIntoView,
        });

        let mut me = Self {
            role,
            session_uuid,
            jsonl_path,
            state,
            pane_configuration,
            focus_handle: None,
            scroll_state,
            selection_handle,
            selected_text,
            expanded_tool_results: HashSet::new(),
            _watcher: None,
            _watch_stream: None,
        };
        me.start_file_watcher(ctx);
        me
    }

    fn start_file_watcher(&mut self, ctx: &mut ViewContext<Self>) {
        let (tx, rx) = mpsc::unbounded::<()>();
        let path = self.jsonl_path.clone();
        let watcher = match new_debouncer(
            Duration::from_millis(FILE_WATCH_DEBOUNCE_MS),
            None,
            move |_result| {
                // Coalesce all event details into a single "something changed"
                // signal — the receiver always re-reads the file end-to-end.
                let _ = tx.unbounded_send(());
            },
        ) {
            Ok(mut debouncer) => match debouncer.watch(&path, RecursiveMode::NonRecursive) {
                Ok(()) => Some(debouncer),
                Err(err) => {
                    log::warn!(
                        "[meridian-agent] failed to watch {}: {err:#}",
                        path.display()
                    );
                    None
                }
            },
            Err(err) => {
                log::warn!("[meridian-agent] failed to create file watcher: {err:#}");
                None
            }
        };
        if watcher.is_none() {
            return;
        }
        self._watcher = watcher;
        let stream = ctx.spawn_stream_local(
            rx,
            |me, _, ctx| {
                me.reload_transcript(ctx);
            },
            |_, _| {},
        );
        self._watch_stream = Some(stream);
    }

    fn reload_transcript(&mut self, ctx: &mut ViewContext<Self>) {
        match MeridianAgentTranscript::load_from_jsonl(&self.jsonl_path) {
            Ok(transcript) => {
                let new_count = transcript.turns.len();
                let old_count = match &self.state {
                    LoadState::Loaded(prev) => prev.turns.len(),
                    LoadState::Error(_) => 0,
                };
                self.state = LoadState::Loaded(transcript);
                if new_count > old_count {
                    self.scroll_state.scroll_to_position(ScrollTarget {
                        position_id: SCROLL_BOTTOM_POSITION_ID.to_string(),
                        mode: ScrollToPositionMode::FullyIntoView,
                    });
                }
            }
            Err(err) => {
                self.state = LoadState::Error(format!("{err:#}"));
            }
        }
        ctx.notify();
    }

    fn copy_selection_to_clipboard(&self, ctx: &mut ViewContext<Self>) {
        let Some(text) = self.selected_text.read().clone().filter(|t| !t.is_empty()) else {
            return;
        };
        ctx.clipboard().write(ClipboardContent::plain_text(text));
    }

    pub fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn session_uuid(&self) -> Uuid {
        self.session_uuid
    }

    pub fn jsonl_path(&self) -> &Path {
        &self.jsonl_path
    }
}

impl Entity for MeridianAgentView {
    type Event = PaneEvent;
}

impl View for MeridianAgentView {
    fn ui_name() -> &'static str {
        "MeridianAgentView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let body: Box<dyn Element> = match &self.state {
            LoadState::Loaded(transcript) if transcript.turns.is_empty() => Text::new(
                format!(
                    "No turns found in transcript for {} ({}).",
                    self.role, self.session_uuid
                ),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(theme.foreground().into())
            .finish(),
            LoadState::Loaded(transcript) => self.render_conversation(transcript, appearance),
            LoadState::Error(message) => Text::new(
                format!("Failed to load transcript: {message}"),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(theme.foreground().into())
            .finish(),
        };

        let selected_text = Arc::clone(&self.selected_text);
        let selectable = SelectableArea::new(
            self.selection_handle.clone(),
            move |args, _, _| {
                *selected_text.write() = args.selection.filter(|s| !s.is_empty());
            },
            body,
        )
        .finish();

        let scrollable = ClippedScrollable::vertical(
            self.scroll_state.clone(),
            selectable,
            ScrollbarWidth::Auto,
            theme.disabled_text_color(theme.background()).into(),
            theme.main_text_color(theme.background()).into(),
            Fill::None,
        )
        .finish();

        Container::new(scrollable)
            .with_background(theme.background())
            .finish()
    }
}

impl MeridianAgentView {
    fn render_conversation(
        &self,
        transcript: &MeridianAgentTranscript,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Min);
        let last_idx = transcript.turns.len().saturating_sub(1);
        for (turn_idx, turn) in transcript.turns.iter().enumerate() {
            let turn_element = self.render_turn(turn_idx, turn, appearance);
            if turn_idx == last_idx {
                // Tag the final turn with the bottom-anchor position id so
                // `scroll_to_position` can jump us to the latest content.
                column = column.with_child(
                    SavePosition::new(turn_element, SCROLL_BOTTOM_POSITION_ID).finish(),
                );
            } else {
                column = column.with_child(turn_element);
            }
        }
        Container::new(column.finish())
            .with_horizontal_padding(20.)
            .with_vertical_padding(16.)
            .finish()
    }

    fn render_turn(
        &self,
        turn_idx: usize,
        turn: &MeridianTurn,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let header_text = format_turn_header(turn);
        let header_color = theme.disabled_text_color(theme.background()).into();
        let header = Text::new(
            header_text,
            appearance.ui_font_family(),
            appearance.monospace_font_size() - 1.,
        )
        .with_color(header_color)
        .finish();

        let mut column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(header);

        for (block_idx, block) in turn.blocks.iter().enumerate() {
            column = column
                .with_child(self.render_block(turn_idx, block_idx, &turn.role, block, appearance));
        }

        Container::new(column.finish())
            .with_margin_bottom(12.)
            .finish()
    }

    fn render_block(
        &self,
        turn_idx: usize,
        block_idx: usize,
        role: &str,
        block: &TurnBlock,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        match block {
            TurnBlock::Text(text) => render_text_block(role, text, appearance),
            TurnBlock::ToolUse {
                name,
                input_summary,
            } => render_tool_use_block(name, input_summary, appearance),
            TurnBlock::ToolResult { content, is_error } => render_tool_result_block(
                content,
                *is_error,
                self.expanded_tool_results.contains(&(turn_idx, block_idx)),
                appearance,
            ),
        }
    }
}

fn format_turn_header(turn: &MeridianTurn) -> String {
    let role_display = match turn.role.as_str() {
        "user" => "You",
        "assistant" => "Assistant",
        "system" => "System",
        other => other,
    };
    match turn.timestamp.as_deref() {
        Some(ts) => format!("{role_display} · {ts}"),
        None => role_display.to_string(),
    }
}

fn render_text_block(role: &str, text: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let text_color = theme.main_text_color(theme.background()).into_solid();
    // Treat assistant text as markdown; render user/system text plain so we don't
    // accidentally mangle a literal `*` in a user prompt as italic.
    let formatted = if role == "assistant" {
        parse_markdown(text).unwrap_or_else(|_| plain_formatted_text(text))
    } else {
        plain_formatted_text(text)
    };
    let inline_code_text_color = theme.terminal_colors().normal.green.into();
    let inline_code_bg_color = theme.background().into_solid();
    let element = FormattedTextElement::new(
        formatted,
        appearance.monospace_font_size(),
        appearance.ai_font_family(),
        appearance.monospace_font_family(),
        text_color,
        Default::default(),
    )
    .with_selection_color(theme.text_selection_color().into_solid())
    .with_line_height_ratio(1.3)
    .with_heading_to_font_size_multipliers(HeadingFontSizeMultipliers {
        h1: 1.55,
        h2: 1.4,
        h3: 1.2,
        ..Default::default()
    })
    .with_inline_code_properties(Some(inline_code_text_color), Some(inline_code_bg_color))
    .set_selectable(true)
    .finish();
    Container::new(element)
        .with_margin_top(2.)
        .with_margin_bottom(4.)
        .finish()
}

fn render_tool_use_block(
    name: &str,
    input_summary: &str,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let dim = theme.disabled_text_color(theme.background()).into();
    let line = if input_summary.is_empty() {
        format!("⚙ {name}")
    } else {
        format!("⚙ {name}: {input_summary}")
    };
    let text = Text::new(
        line,
        appearance.monospace_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(dim)
    .with_selectable(true)
    .finish();
    Container::new(text)
        .with_margin_top(2.)
        .with_margin_bottom(2.)
        .with_margin_left(8.)
        .finish()
}

fn render_tool_result_block(
    content: &str,
    is_error: bool,
    expanded: bool,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let body_color = if is_error {
        theme.terminal_colors().normal.red.into()
    } else {
        theme.disabled_text_color(theme.background()).into()
    };
    let (display_text, total_lines, was_truncated) = if expanded {
        (content.to_owned(), content.lines().count().max(1), false)
    } else {
        fold_tool_result(content)
    };
    let body = Text::new(
        display_text,
        appearance.monospace_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(body_color)
    .with_selectable(true)
    .finish();

    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(body);
    if was_truncated {
        let footer = Text::new(
            format!("… (truncated, full result {total_lines} lines)"),
            appearance.ui_font_family(),
            appearance.monospace_font_size() - 2.,
        )
        .with_color(theme.disabled_text_color(theme.background()).into())
        .finish();
        column = column.with_child(footer);
    }
    Container::new(column.finish())
        .with_margin_top(2.)
        .with_margin_bottom(4.)
        .with_margin_left(16.)
        .finish()
}

/// Fallback: wrap a literal string into a single-line `FormattedText` with no markdown
/// styling. Used for user/system text and for assistant text that fails to parse.
fn plain_formatted_text(text: &str) -> FormattedText {
    FormattedText::new([FormattedTextLine::Line(vec![
        FormattedTextFragment::plain_text(text.to_owned()),
    ])])
}

impl TypedActionView for MeridianAgentView {
    type Action = MeridianAgentAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            MeridianAgentAction::CopySelection => self.copy_selection_to_clipboard(ctx),
        }
    }
}

/// Actions dispatched to a `MeridianAgentView`. Currently only `CopySelection`
/// (Cmd+C while the pane has focus + a selection is active).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeridianAgentAction {
    CopySelection,
}

impl BackingView for MeridianAgentView {
    type PaneHeaderOverflowMenuAction = ();
    type CustomAction = ();
    type AssociatedData = ();

    fn handle_pane_header_overflow_menu_action(
        &mut self,
        _action: &Self::PaneHeaderOverflowMenuAction,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(PaneEvent::Close);
    }

    fn focus_contents(&mut self, _ctx: &mut ViewContext<Self>) {}

    fn render_header_content(
        &self,
        _ctx: &HeaderRenderContext<'_>,
        _app: &AppContext,
    ) -> HeaderContent {
        HeaderContent::simple(format!("Agent: {}", self.role))
    }

    fn set_focus_handle(&mut self, focus_handle: PaneFocusHandle, _ctx: &mut ViewContext<Self>) {
        self.focus_handle = Some(focus_handle);
    }
}

/// Pane wrapper that satisfies [`PaneContent`] for the pane group machinery.
pub struct MeridianAgentPane {
    view: ViewHandle<PaneView<MeridianAgentView>>,
    pane_configuration: ModelHandle<PaneConfiguration>,
}

impl MeridianAgentPane {
    pub fn new<V: View>(
        role: String,
        session_uuid: Uuid,
        jsonl_path: PathBuf,
        ctx: &mut ViewContext<V>,
    ) -> Self {
        let agent_view = ctx.add_typed_action_view(|ctx| {
            MeridianAgentView::new(role, session_uuid, jsonl_path, ctx)
        });
        let pane_configuration = agent_view.as_ref(ctx).pane_configuration();
        let view = ctx.add_typed_action_view(|ctx| {
            let pane_id = PaneId::from_meridian_agent_pane_ctx(ctx);
            PaneView::new(pane_id, agent_view, (), pane_configuration.clone(), ctx)
        });
        Self {
            view,
            pane_configuration,
        }
    }
}

impl PaneContent for MeridianAgentPane {
    fn id(&self) -> PaneId {
        PaneId::from_meridian_agent_pane_view(&self.view)
    }

    fn snapshot(&self, app: &AppContext) -> LeafContents {
        let inner = self.view.as_ref(app).child(app);
        let inner = inner.as_ref(app);
        LeafContents::MeridianAgent {
            role: inner.role.clone(),
            session_uuid: inner.session_uuid,
            jsonl_path: inner.jsonl_path.clone(),
        }
    }

    fn attach(
        &self,
        _group: &PaneGroup,
        focus_handle: PaneFocusHandle,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        self.view
            .update(ctx, |view, ctx| view.set_focus_handle(focus_handle, ctx));
        let pane_id = self.id();
        let child = self.view.as_ref(ctx).child(ctx);
        ctx.subscribe_to_view(&child, move |group, _, event, ctx| {
            group.handle_pane_event(pane_id, event, ctx);
        });
    }

    fn detach(
        &self,
        _group: &PaneGroup,
        _detach_type: DetachType,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        let child = self.view.as_ref(ctx).child(ctx);
        ctx.unsubscribe_to_view(&child);
    }

    fn has_application_focus(&self, ctx: &mut ViewContext<PaneGroup>) -> bool {
        self.view.is_self_or_child_focused(ctx)
    }

    fn focus(&self, ctx: &mut ViewContext<PaneGroup>) {
        self.view
            .as_ref(ctx)
            .child(ctx)
            .update(ctx, BackingView::focus_contents);
    }

    fn shareable_link(
        &self,
        _ctx: &mut ViewContext<PaneGroup>,
    ) -> Result<ShareableLink, ShareableLinkError> {
        Ok(ShareableLink::Base)
    }

    fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    fn is_pane_being_dragged(&self, ctx: &AppContext) -> bool {
        self.view.as_ref(ctx).is_being_dragged()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// 4-turn fixture: user → assistant text+tool_use → user tool_result → assistant.
    /// Mirrors the real Claude Code JSONL shape captured from a wt2 session.
    const FIXTURE: &str = concat!(
        r#"{"type":"file-history-snapshot","messageId":"snap-1"}"#,
        "\n",
        r#"{"parentUuid":null,"type":"user","message":{"role":"user","content":"hello"},"uuid":"u1","timestamp":"2026-05-15T12:00:00Z"}"#,
        "\n",
        r#"{"parentUuid":"u1","type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi! Let me check."},{"type":"tool_use","name":"Bash","id":"t1","input":{"command":"git status --short"}}]},"uuid":"a1","timestamp":"2026-05-15T12:00:01Z"}"#,
        "\n",
        r#"{"parentUuid":"a1","type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"M file.rs","is_error":false}]},"uuid":"u2","timestamp":"2026-05-15T12:00:02Z"}"#,
        "\n",
        r#"{"parentUuid":"u2","type":"assistant","message":{"role":"assistant","content":"Done."},"uuid":"a2","timestamp":"2026-05-15T12:00:03Z"}"#,
        "\n",
    );

    #[test]
    fn load_fixture_jsonl_extracts_four_turns_with_blocks_and_roles() {
        let tmp = TempDir::new().unwrap();
        let jsonl = tmp.path().join("session.jsonl");
        fs::write(&jsonl, FIXTURE).unwrap();

        let transcript = MeridianAgentTranscript::load_from_jsonl(&jsonl).unwrap();
        assert_eq!(transcript.turns.len(), 4, "{:?}", transcript.turns);

        // Turn 0: user with plain text.
        assert_eq!(transcript.turns[0].role, "user");
        assert_eq!(transcript.turns[0].blocks.len(), 1);
        assert!(matches!(
            &transcript.turns[0].blocks[0],
            TurnBlock::Text(t) if t == "hello"
        ));
        assert_eq!(
            transcript.turns[0].timestamp.as_deref(),
            Some("2026-05-15T12:00:00Z")
        );

        // Turn 1: assistant with text + tool_use.
        assert_eq!(transcript.turns[1].role, "assistant");
        assert_eq!(transcript.turns[1].blocks.len(), 2);
        assert!(matches!(
            &transcript.turns[1].blocks[0],
            TurnBlock::Text(t) if t.contains("Hi! Let me check.")
        ));
        match &transcript.turns[1].blocks[1] {
            TurnBlock::ToolUse {
                name,
                input_summary,
            } => {
                assert_eq!(name, "Bash");
                assert_eq!(input_summary, "git status --short");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }

        // Turn 2: user with tool_result.
        assert_eq!(transcript.turns[2].role, "user");
        match &transcript.turns[2].blocks[0] {
            TurnBlock::ToolResult { content, is_error } => {
                assert_eq!(content, "M file.rs");
                assert!(!*is_error);
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }

        // Turn 3: assistant with plain text.
        assert_eq!(transcript.turns[3].role, "assistant");
        assert_eq!(
            transcript.turns[3].plain_text(),
            "Done.",
            "plain_text concatenates blocks"
        );
    }

    #[test]
    fn load_missing_jsonl_returns_empty_transcript() {
        let tmp = TempDir::new().unwrap();
        let jsonl = tmp.path().join("missing.jsonl");
        let transcript = MeridianAgentTranscript::load_from_jsonl(&jsonl).unwrap();
        assert!(transcript.turns.is_empty());
    }

    #[test]
    fn fold_tool_result_preserves_short_output() {
        let short = "line 1\nline 2";
        let (preview, total, truncated) = fold_tool_result(short);
        assert_eq!(preview, short);
        assert_eq!(total, 2);
        assert!(!truncated);
    }

    #[test]
    fn fold_tool_result_truncates_long_output() {
        let long = (0..50)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (preview, total, truncated) = fold_tool_result(&long);
        assert_eq!(total, 50);
        assert!(truncated);
        assert_eq!(
            preview.lines().count(),
            TOOL_RESULT_FOLD_PREVIEW_LINES,
            "preview = {preview:?}"
        );
    }

    #[test]
    fn summarize_tool_input_prefers_command_field() {
        let input = serde_json::json!({"command": "ls -la /tmp", "extra": "ignored"});
        let summary = summarize_tool_input(Some(&input));
        assert_eq!(summary, "ls -la /tmp");
    }

    #[test]
    fn summarize_tool_input_truncates_long_values() {
        let long = "x".repeat(200);
        let input = serde_json::json!({"command": long});
        let summary = summarize_tool_input(Some(&input));
        assert!(summary.ends_with('…'));
        assert!(
            summary.chars().count() <= TOOL_USE_INPUT_PREVIEW_CHARS + 1,
            "{summary:?}"
        );
    }

    #[test]
    fn turn_block_plain_text_handles_each_variant() {
        assert_eq!(TurnBlock::Text("hi".into()).as_plain_text(), "hi");
        assert_eq!(
            TurnBlock::ToolUse {
                name: "Read".into(),
                input_summary: "/tmp/x".into()
            }
            .as_plain_text(),
            "[tool: Read] /tmp/x"
        );
        assert_eq!(
            TurnBlock::ToolResult {
                content: "ok".into(),
                is_error: false
            }
            .as_plain_text(),
            "[tool result] ok"
        );
        assert_eq!(
            TurnBlock::ToolResult {
                content: "boom".into(),
                is_error: true
            }
            .as_plain_text(),
            "[tool result, error] boom"
        );
    }
}

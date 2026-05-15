//! `MeridianAgent` pane — read-only display of an existing Claude Code session JSONL.
//!
//! Phase 3b-A: foundational pane for the orchestration UI. Renders a transcript on disk
//! (`~/.claude/projects/<encoded_cwd>/<uuid>.jsonl`) as a chat-style conversation. No
//! file watching, no input, no server sync. Live-refresh, input, and the launcher land
//! in 3b-B/C/D.
//!
//! Architecture: deliberately bypasses the [`crate::ai::agent::conversation::AIConversation`]
//! / `BlocklistAIHistoryModel` path because that infra is server-synced via gRPC to
//! Anthropic and registers conversations globally. We want pure local-only rendering, so
//! we parse the JSONL into a lightweight [`MeridianAgentTranscript`] and render directly
//! with `warpui` primitives.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp_core::ui::appearance::Appearance;
use warpui::elements::{Container, CrossAxisAlignment, Flex, MainAxisSize, ParentElement, Text};
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

/// One displayable turn extracted from a Claude Code JSONL transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeridianTurn {
    /// Role label as shown in the UI: typically `"user"`, `"assistant"`, or `"system"`.
    pub role: String,
    /// Plain-text body extracted from the JSONL entry. Tool-use / tool-result blocks
    /// are summarized inline with simple `[tool: name]` markers in this round.
    pub text: String,
    /// ISO-8601 timestamp string straight from the JSONL `timestamp` field, if present.
    pub timestamp: Option<String>,
}

/// Lightweight transcript representation used for read-only rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeridianAgentTranscript {
    pub turns: Vec<MeridianTurn>,
}

impl MeridianAgentTranscript {
    /// Read the JSONL at `path` and convert recognized entries to displayable turns.
    ///
    /// Recognized top-level `type` values: `"user"`, `"assistant"`, `"system"`. Other
    /// entry types (e.g. `"file-history-snapshot"`) are skipped silently. Malformed
    /// JSONL lines are skipped by the underlying reader with a log warning.
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
            let text = extract_text(entry);
            if text.is_empty() {
                continue;
            }
            let timestamp = entry
                .get("timestamp")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            turns.push(MeridianTurn {
                role,
                text,
                timestamp,
            });
        }
        Ok(Self { turns })
    }
}

/// Pull a printable string out of a Claude JSONL entry's `message.content` field
/// (or the top-level `content` for `system` entries). Content can be a plain string
/// or a list of typed blocks (`text`, `tool_use`, `tool_result`); we render
/// each block inline.
fn extract_text(entry: &serde_json::Value) -> String {
    if let Some(content) = entry
        .get("message")
        .and_then(|m| m.get("content"))
        .or_else(|| entry.get("content"))
    {
        return render_content(content);
    }
    String::new()
}

fn render_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) => {
            let mut out = String::new();
            for block in blocks {
                if !out.is_empty() {
                    out.push('\n');
                }
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            out.push_str(text);
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        out.push_str(&format!("[tool: {name}]"));
                    }
                    "tool_result" => {
                        let result_text =
                            block.get("content").map(render_content).unwrap_or_default();
                        out.push_str("[tool result] ");
                        out.push_str(&result_text);
                    }
                    other => {
                        out.push_str(&format!("[{other}]"));
                    }
                }
            }
            out
        }
        other => other.to_string(),
    }
}

/// Loaded state for the pane.
#[derive(Debug, Clone)]
enum LoadState {
    Loaded(MeridianAgentTranscript),
    Error(String),
}

/// View that owns the loaded transcript and renders it.
pub struct MeridianAgentView {
    role: String,
    session_uuid: Uuid,
    jsonl_path: PathBuf,
    state: LoadState,
    pane_configuration: ModelHandle<PaneConfiguration>,
    focus_handle: Option<PaneFocusHandle>,
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
        let state = match MeridianAgentTranscript::load_from_jsonl(&jsonl_path) {
            Ok(transcript) => LoadState::Loaded(transcript),
            Err(err) => LoadState::Error(format!("{err:#}")),
        };
        Self {
            role,
            session_uuid,
            jsonl_path,
            state,
            pane_configuration,
            focus_handle: None,
        }
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
            LoadState::Loaded(transcript) => {
                let mut column = Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Start)
                    .with_main_axis_size(MainAxisSize::Min);
                for turn in &transcript.turns {
                    column = column.with_child(render_turn(turn, appearance));
                }
                column.finish()
            }
            LoadState::Error(message) => Text::new(
                format!("Failed to load transcript: {message}"),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .with_color(theme.foreground().into())
            .finish(),
        };

        Container::new(body)
            .with_background(theme.background())
            .with_horizontal_padding(20.)
            .with_vertical_padding(16.)
            .finish()
    }
}

fn render_turn(turn: &MeridianTurn, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let header_text = match turn.timestamp.as_deref() {
        Some(ts) => format!("{} · {ts}", turn.role),
        None => turn.role.clone(),
    };
    let header = Text::new(
        header_text,
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(theme.foreground().into())
    .finish();
    let body = Text::new(
        turn.text.clone(),
        appearance.monospace_font_family(),
        appearance.monospace_font_size(),
    )
    .with_color(theme.foreground().into())
    .finish();
    let column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(header)
        .with_child(body)
        .finish();
    Container::new(column)
        .with_horizontal_padding(0.)
        .with_margin_bottom(12.)
        .finish()
}

impl TypedActionView for MeridianAgentView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
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

    /// 3-turn fixture: user → assistant (with text + tool_use) → assistant text.
    /// Mirrors the real Claude Code JSONL shape captured from a wt2 session.
    const FIXTURE: &str = concat!(
        r#"{"type":"file-history-snapshot","messageId":"snap-1"}"#,
        "\n",
        r#"{"parentUuid":null,"type":"user","message":{"role":"user","content":"hello"},"uuid":"u1","timestamp":"2026-05-15T12:00:00Z"}"#,
        "\n",
        r#"{"parentUuid":"u1","type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi! Let me check."},{"type":"tool_use","name":"Read","id":"t1"}]},"uuid":"a1","timestamp":"2026-05-15T12:00:01Z"}"#,
        "\n",
        r#"{"parentUuid":"a1","type":"assistant","message":{"role":"assistant","content":"Done."},"uuid":"a2","timestamp":"2026-05-15T12:00:02Z"}"#,
        "\n",
    );

    #[test]
    fn load_fixture_jsonl_extracts_three_turns_with_roles_and_text() {
        let tmp = TempDir::new().unwrap();
        let jsonl = tmp.path().join("session.jsonl");
        fs::write(&jsonl, FIXTURE).unwrap();

        let transcript = MeridianAgentTranscript::load_from_jsonl(&jsonl).unwrap();
        assert_eq!(transcript.turns.len(), 3, "{:?}", transcript.turns);

        assert_eq!(transcript.turns[0].role, "user");
        assert_eq!(transcript.turns[0].text, "hello");
        assert_eq!(
            transcript.turns[0].timestamp.as_deref(),
            Some("2026-05-15T12:00:00Z")
        );

        assert_eq!(transcript.turns[1].role, "assistant");
        assert!(
            transcript.turns[1].text.contains("Hi! Let me check."),
            "{}",
            transcript.turns[1].text
        );
        assert!(
            transcript.turns[1].text.contains("[tool: Read]"),
            "{}",
            transcript.turns[1].text
        );

        assert_eq!(transcript.turns[2].role, "assistant");
        assert_eq!(transcript.turns[2].text, "Done.");
    }

    #[test]
    fn load_missing_jsonl_returns_empty_transcript() {
        let tmp = TempDir::new().unwrap();
        let jsonl = tmp.path().join("missing.jsonl");
        let transcript = MeridianAgentTranscript::load_from_jsonl(&jsonl).unwrap();
        assert!(transcript.turns.is_empty());
    }
}

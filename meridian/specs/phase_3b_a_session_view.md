# Spec — Phase 3b-A: `MeridianAgent` pane — load + display an existing Claude Code session

> First dispatchable sub-MVP of Phase 3b orchestration UI. Adds a new pane type that displays an existing Claude Code session's transcript as a chat-style conversation, read-only. Foundation for live-refresh (3b-B) + input (3b-C) which come after.

## Purpose
Prove we can render an existing `claude` session's transcript inside a Hyperdrive pane as something other than a terminal grid. Specifically: load the JSONL at `~/.claude/projects/<encoded_cwd>/<session_uuid>.jsonl`, hydrate it into an `AIConversation`, and display via the existing agent-view rendering infrastructure.

After 3b-A: open Hyperdrive → trigger a debug command → see Manager's (or any builder's) conversation rendered as a chat, not a terminal.

## Architecture pattern to follow

**Clone target:** `/app/src/pane_group/pane/ai_document_pane.rs` is the simplest existing pane to use as a template. Read it first.

**Reuse, don't rebuild:**
- `/app/src/ai/agent_sdk/driver/harness/claude_code.rs` — the existing Claude Code transcript handler. Already reads `~/.claude/projects/<encoded_cwd>/<uuid>.jsonl` and deserializes via `ClaudeTranscriptEnvelope`.
- `/app/src/ai/agent_sdk/driver/harness/claude_transcript.rs` — the JSONL parsing primitives (verify exact filename when reading)
- `/app/src/ai/agent/conversation.rs` — `AIConversation` struct + builder helpers
- `/app/src/ai/blocklist/agent_view/agent_view_block.rs` — the existing render code for displaying a conversation; we reuse / adapt this

## Scope (in this dispatch)

### 1. New pane type
- Add `MeridianAgent` variant to `IPaneType` enum at `/app/src/pane_group/pane/mod.rs` lines 138–162
- Update `IPaneType::Display` impl for the new variant
- Update any `match` statements that exhaustively cover `IPaneType` (compiler will surface these)

### 2. New `LeafContents` variant
- Add `MeridianAgent { role: String, session_uuid: Uuid, jsonl_path: PathBuf }` variant to `LeafContents` enum at `/app/src/app_state.rs` lines 120–148
- Update `is_persisted()` match — return `true` (the session JSONL persists across restarts)
- Update any other exhaustive matches on `LeafContents`

### 3. New file: `/app/src/pane_group/pane/meridian_agent_pane.rs`
Implements the `BackingView` + `PaneContent` traits. Structure to mirror `ai_document_pane.rs`. Key methods:

```rust
pub struct MeridianAgentPane {
    role: String,
    session_uuid: Uuid,
    jsonl_path: PathBuf,
    conversation: Option<AIConversation>,  // populated on load
    load_error: Option<String>,            // set if JSONL load fails
}

impl MeridianAgentPane {
    pub fn new(role: String, session_uuid: Uuid, jsonl_path: PathBuf, ctx: &mut ViewContext<Self>) -> Self {
        // Spawn a task to load the JSONL → AIConversation
        // Use ClaudeTranscriptEnvelope from agent_sdk/driver/harness/
        // Update self.conversation on success; self.load_error on failure
    }
}

impl BackingView for MeridianAgentPane {
    // render(): if conversation loaded → render via agent_view_block primitives
    //          if loading → spinner / "Loading session..."
    //          if error → error message
    // ... other BackingView methods as ai_document_pane.rs does them
}

impl PaneContent for MeridianAgentPane {
    fn title(&self) -> String { format!("Agent: {}", self.role) }
    // serialization roundtrip via LeafContents::MeridianAgent
}
```

**Key design decisions:**
- Read-only for this round. No input widget yet (3b-C).
- No file watcher yet (3b-B). One-time load on construction.
- Use existing agent_view_block render code for the conversation body — don't write a new renderer. Wrap if needed.
- The hydrated `AIConversation` is **local-only**, NOT synced to Anthropic's server. The existing claude_code harness path may want to sync; we explicitly want to skip the server sync (we're rendering disk-only). If the existing harness forces sync, we'll need to read the JSONL directly and build `AIConversation` manually — Builder makes that judgment call.

### 4. Pane factory wire-up
- Update `/app/src/pane_group/mod.rs` factory to handle `LeafContents::MeridianAgent { ... }` → instantiate `MeridianAgentPane`
- Update any pane-creation dispatch tables / match arms

### 5. Debug entry point (temporary; replaced by 3b-D's launcher later)
Add a temporary command or hardcoded test that lets us open a `MeridianAgent` pane for development. Options Builder can choose between:

**Option A (simplest):** A hardcoded test that opens a pane for `builder-1` on startup if env var `MERIDIAN_DEBUG_OPEN_AGENT=builder-1` is set
**Option B (cleaner):** Add to command palette as `meridian: open agent` with a role-name prompt. Reads `~/.meridian/agents/<role>/{session-id,cwd}` to construct the pane args
**Option C (Builder's choice):** Any equivalent that lets us verify the pane opens + renders

Builder picks the option that fits the codebase's existing patterns. Document the choice.

### 6. Unit test (one is sufficient for this round)
- A fixture JSONL with 3-5 turns (user/assistant alternating, maybe one tool call)
- Test loads the fixture into `AIConversation` via the same code path the pane uses
- Asserts exchanges count, role assignments, text content roundtrip

### 7. Gates
```
cargo check -p warp_terminal -p warpui
cargo check --bin warp-oss
cargo clippy --bin warp-oss --all-targets
cargo test -p warp (or whichever crate the pane lives in)
```

All must pass. The new code should not regress existing warp-oss build.

## Out of scope (deferred to later sub-MVPs)
- **File watching / live refresh** (3b-B)
- **User input handling** (3b-C)
- **Launcher / agent registry sidebar** (3b-D)
- **Status indicators, styling polish, theming work** (3b-E)
- **Sync to Anthropic server** — explicitly NOT wanted; we're rendering local JSONL only
- **Streaming responses** — read-only this round
- **Manager dispatch script integration** — that's 3b-C
- **Per-message styling differentiation** (chat-bubble vs block) — minimal styling for now; design polish in 3b-E

## Files Builder will touch
- `/app/src/pane_group/pane/mod.rs` (IPaneType enum, Display impl)
- `/app/src/app_state.rs` (LeafContents enum, is_persisted match)
- `/app/src/pane_group/pane/meridian_agent_pane.rs` (NEW)
- `/app/src/pane_group/mod.rs` (factory)
- Possibly 1-2 other files where exhaustive matches on the enums live (compiler tells you)
- A fixture JSONL somewhere under `/app/src/pane_group/pane/test_fixtures/` (NEW)

## Files Builder will NOT touch
- `/app/src/ai/agent_sdk/driver/harness/*` — read but don't modify (existing harness logic stays)
- `/app/src/ai/blocklist/agent_view/agent_view_block.rs` — reuse, don't modify (if it needs refactoring, defer to 3b-E)
- Anything in `/crates/meridian_*` (no changes to those crates this round)
- `/app/Cargo.toml` (unless a new dep is actually needed, which I don't think it is — `Uuid`/`PathBuf` are already pulled in)

## Reporting

Return EXACTLY one JSON object as final response:

```json
{
  "status": "ok" or "error",
  "commit_sha": "<short SHA on wt1>",
  "files_changed": [...],
  "files_added": [...],
  "debug_entry_point": "<how to open a MeridianAgent pane for testing — option A/B/C from spec>",
  "cargo_check_warp_oss": "ok" or "fail",
  "cargo_clippy_warp_oss": "ok" or "fail",
  "test_count": <number of new tests passing>,
  "notes": "<surprises, design choices, unverified assumptions, deferred items>",
  "follow_up_for_manager": "<anything Manager should investigate or decide before 3b-B is dispatched>"
}
```

On error: `{"status": "error", "step": "<which>", "reason": "<root cause>", "files_already_modified": [...]}`.

## Notes for Builder
- The recon report flagged that Warp's existing AI conversation infra is **server-synced via gRPC to Anthropic**. For our purposes we want **local-only rendering**. If the existing `ClaudeTranscriptEnvelope` → `AIConversation` path forces server sync, you may need to either:
  - Use a lower-level parser that just reads JSONL into a `Vec<Exchange>` directly
  - Build a "local-only" `AIConversation` constructor that skips server registration
  - Document the choice and flag in `follow_up_for_manager`
- Reading `/app/src/ai/agent_sdk/driver/harness/claude_code.rs` first will tell you which path is feasible.
- This is the foundation for the whole orchestration UI. Get the architecture right; the next 4 sub-MVPs depend on the patterns you set here. **Read before write** — this is exactly the kind of round where insight 01's antidote matters most.
- Estimated 3–5 hours of Builder work, ~$5–10 cost. Use Bash, Read, Edit freely (permissions skipped at workspace level).

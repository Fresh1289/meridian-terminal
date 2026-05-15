# Phase 3b — Orchestration UI (Overall Plan)

> Real Warp pane types for visible agent orchestration. Replaces the shell-script interim (`meridian-watch.sh`) with native UI surfaces. Multi-session work, sliced into 5 sub-MVPs (3b-A through 3b-E). Each sub-MVP is one Builder dispatch.

## Architecture (from recon 2026-05-15)

The fork's existing AI conversation infrastructure does most of the heavy lifting:

- **Conversation data model:** `AIConversation` in `/app/src/ai/agent/conversation.rs` — server-synced, in-memory, backed by `BlocklistAIHistoryModel` at `/app/src/ai/blocklist/history_model.rs`
- **Claude Code transcript parser already exists:** `/app/src/ai/agent_sdk/driver/harness/claude_code.rs` reads `~/.claude/projects/<encoded_cwd>/<uuid>.jsonl`, deserializes via `ClaudeTranscriptEnvelope`, hydrates into `AIConversation`. **We plug into this, not rebuild it.**
- **Existing agent view rendering:** `/app/src/ai/blocklist/agent_view/agent_view_block.rs` — block-based, not chat-bubble. We adapt or extend.
- **Pane registration template:** `/app/src/pane_group/pane/ai_document_pane.rs` is the clone target for the new pane file
- **`IPaneType` enum** at `/app/src/pane_group/pane/mod.rs` lines 138–162
- **`LeafContents` enum** at `/app/src/app_state.rs` lines 120–148
- **`notify-debouncer-full`** already in workspace deps but unused — we pioneer file-watching for live JSONL refresh

## Sub-MVP slicing

### Phase 3b-A — `MeridianAgent` pane: load + display (read-only)
**Goal:** Open a new pane type that displays an existing Claude Code session's JSONL as a chat-style conversation. No live refresh, no input. The minimum proof that we can render an existing claude session inside Hyperdrive as a non-terminal pane.

- New `IPaneType::MeridianAgent` variant
- New `LeafContents::MeridianAgent { role: String, session_uuid: Uuid, jsonl_path: PathBuf }`
- New `/app/src/pane_group/pane/meridian_agent_pane.rs` implementing `BackingView`
- Pane loads the JSONL via existing `ClaudeTranscriptEnvelope` parser, builds an `AIConversation`, renders using existing agent view block patterns (`agent_view_block.rs`)
- For now: hardcoded open path — e.g., a debug command `meridian: open agent <role>` that reads `~/.meridian/agents/<role>/{session-id,cwd}` and opens a pane
- Pane factory + `LeafContents::is_persisted()` match arms updated
- Tests at the unit level (load a fixture JSONL, assert AIConversation has expected exchanges)

**Out of scope for 3b-A:** input, live refresh, sidebar, theming polish, status indicators.

### Phase 3b-B — Live JSONL watching
**Goal:** Pane auto-refreshes when new turns are appended to its JSONL (by Manager's `meridian-dispatch.sh` calls or by another claude process).

- Spawn a model-level background task using `ModelContext::spawn_task()` (warpui pattern)
- Use `notify-debouncer-full` to watch the JSONL path
- On change, re-read new lines, parse new exchanges, append to in-memory `AIConversation`, emit event to subscribers
- View re-renders on the event

**Open question for 3b-B:** is `notify-debouncer-full`'s watcher channel compatible with warpui's task system? If not, we either bridge it or use a polling loop on `tokio::time::interval`.

### Phase 3b-C — Input → new user turn
**Goal:** User types in the pane, hits Enter, that text becomes a new user turn in the session.

- Add text input widget at the bottom of the pane
- On submit, shell out to `claude --resume <uuid> --print "<text>" --output-format json` (or the equivalent `meridian-dispatch.sh`)
- The dispatched response appends to the JSONL, which the watcher from 3b-B picks up — closing the loop
- Spinner while dispatch is in flight

**Out of scope for 3b-C:** streaming responses (the dispatched call is one-shot via `--print`). Manager input flows through `meridian-dispatch.sh`; CTO typing in a Builder pane directly might go via a different path (TBD).

### Phase 3b-D — Agent launcher
**Goal:** Easy ways to open agent panes — command palette ("Open Manager", "Open Builder 1", etc.) + maybe a left-rail sidebar showing registered agents.

- Scan `~/.meridian/agents/*/` to enumerate registered agents
- Add command palette commands (per Warp's command palette infra)
- Optional sidebar pane that lists agents (deferred sub-piece — sidebar pattern doesn't exist in the codebase yet)

### Phase 3b-E — Polish
- Visual differentiation: user vs assistant message styling (chat-bubble or block, TBD with CTO design input)
- Status indicators (working / idle / errored)
- Theming (use existing Appearance system)
- Maybe: cross-pane status (Manager pane shows which builders are working right now)

## Sequencing
1. 3b-A first (foundational; everything builds on this)
2. 3b-B next (delivers "visible" since dispatches show up live)
3. 3b-C (closes the input loop — now you can actually USE a pane to chat with an agent)
4. 3b-D (ergonomic launcher)
5. 3b-E (polish)

After 3b-A+B+C, the orchestration UI is functional. D+E are quality-of-life.

## Files this plan touches (across all sub-MVPs)
- `/app/src/pane_group/pane/mod.rs` (IPaneType enum)
- `/app/src/pane_group/pane/meridian_agent_pane.rs` (new file)
- `/app/src/app_state.rs` (LeafContents enum)
- `/app/src/pane_group/mod.rs` (pane factory)
- `/app/src/ai/blocklist/agent_view/agent_view_block.rs` (possibly reused)
- Possibly new model crate `meridian_agent_session` if file-watching logic gets substantial

## Constraints (still in effect)
- Manager NEVER edits `.rs` / crate-level `Cargo.toml` — all sub-MVPs are Builder dispatches
- Cherry-pick wt1 → main per established pattern
- Catch-up merges for wt2/wt3 after each sub-MVP lands
- Phase 2a script bridge stays alive — it's the input plumbing for 3b-C and a useful fallback always

## Estimated total effort
- 3b-A: ~3-5 hours Builder
- 3b-B: ~2-4 hours Builder
- 3b-C: ~2-3 hours Builder
- 3b-D: ~2-3 hours Builder
- 3b-E: ~3-6 hours Builder
- Total: ~12-20 hours of Builder work, spread across 2-4 evenings

## What this plan replaces / supersedes
- `meridian-watch.sh` (still useful as a debug tool; not removed; just no longer the primary "visible relay" answer)
- `meridian-dispatch.sh` (stays as the dispatch primitive; 3b-C UI calls into it)

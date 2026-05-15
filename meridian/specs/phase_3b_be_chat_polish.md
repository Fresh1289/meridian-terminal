# Spec — Phase 3b-B + 3b-E: Live refresh + chat-quality rendering polish

> Combined dispatch that takes the read-only MeridianAgent pane from Phase 3b-A and makes it feel like a real chat surface. Live refresh on JSONL appends, text selection / scroll / copy, markdown rendering, visual differentiation between turn types, foldable long tool_results. Input handling (3b-C) is a separate later dispatch.

## Purpose

Today the pane works but is debug-grade — plain text dump, no selection, no scroll, no formatting, no live updates. After this round:

- Manager dispatches into Builder N → Builder N's MeridianAgent pane updates **live**, in real time, message-by-message
- Assistant text renders as **clean markdown** (bold, italic, code blocks with syntax highlighting, headings, lists)
- **Text is selectable + copyable** + the pane scrolls like a normal chat
- User / assistant / tool_use / tool_result get **visually distinct** styling so the conversation is easy to scan
- Long tool_results (e.g., `cat MERIDIAN.md` dumping 200 lines) **fold to a summary** with click-to-expand

The result feels like watching two humans text, not like reading a JSONL dump.

## Reference docs you (Builder) already wrote
- Your Phase 3a-1 work and revert at `/app/src/appearance.rs`, `/app/src/settings/font.rs`
- Your Phase 3b-A work at `/app/src/pane_group/pane/meridian_agent_pane.rs` — this round extends that file
- Phase 3b overall plan: `meridian/specs/phase_3b_orchestration_ui.md`
- Phase 3b-A spec: `meridian/specs/phase_3b_a_session_view.md`

## Scope

### 1. Live refresh — file watcher on the JSONL (Phase 3b-B core)

- Use `notify-debouncer-full` (already in workspace deps; pioneered here)
- Spawn a background watcher per `MeridianAgentPane` instance, scoped to its specific JSONL path
- On debounced file change, re-read new turns from the JSONL (append-only — track byte offset of last read, only parse new bytes), update the in-memory `MeridianAgentTranscript`, emit event to the view to redraw
- Debounce window ~100–250 ms (avoid hammering on rapid appends from a streaming agent response)
- Watcher tears down cleanly when the pane is closed

**Architecture pattern to study before implementing:** look for any existing async-task patterns in warpui's pane code. The recon flagged `ModelContext::spawn_task()` as the likely path. If `notify-debouncer-full` channel-based event delivery doesn't compose with warpui's task system, document the bridge approach in your follow-up notes.

### 2. Text selection, copy, scroll (Phase 3b-E foundational UX)

- The pane's rendered text must be **selectable** (drag to select, Cmd+C to copy)
- Find the appropriate warpui text widget / `SelectableText` primitive (almost certainly exists — most Warp panes have selectable content). Clone the pattern from `terminal/` or `ai/blocklist/agent_view/` panes
- Wrap the pane's content in a scrollable container so long conversations scroll naturally (Cmd+Down / scrollwheel / etc.)
- Conversation grows from top → bottom; new messages appear at the bottom; auto-scroll to bottom on new turn UNLESS user has scrolled up (preserve scroll position on append if reading history)

### 3. Markdown rendering (Phase 3b-E polish)

Render assistant text as **clean polished markdown**, NOT plain text. Specifically:

- **Headings** (`# H1`, `## H2`, etc.) — larger/bolder, distinct hierarchy
- **Bold / italic** (`**bold**`, `*italic*`)
- **Code blocks** (` ``` ` fences) — monospace font (Hack), light background, syntax-highlighted by language hint if present (Rust/bash/etc.). Use `syntect` if it's already in workspace deps; otherwise pick the simplest existing path (Warp's terminal block code rendering probably has something we can reuse)
- **Inline code** (`` `code` ``) — monospace, light background pill
- **Lists** (bullets + numbered) — proper indentation
- **Links** — visually distinguishable, no need to be clickable in v1

NOT in scope for this round: LaTeX math typesetting (`$x^2$` etc.), tables, images, mermaid diagrams. Document any of those if you encounter them in transcripts — defer to a future polish round.

**Library choice:** if there's no existing markdown renderer in the warp workspace, use `pulldown-cmark` (industry-standard, lightweight, parser-only — you'd render to warpui's text/container primitives manually). Document the choice.

### 4. Visual differentiation by turn type

Each turn type renders differently:

- **User prompt** — distinct left bubble / indent, soft accent color (e.g., theme.foreground subdued), prefix "You" or similar
- **Assistant text** — distinct styling, primary text color, prefix "Assistant" / role name
- **Tool use** (`tool_use` blocks) — compact one-line summary like `🛠 Bash: git status --short` (icon + tool name + first 80 chars of input)
- **Tool result** — compact, indented under the tool use, color-coded if `is_error: true`

Use the existing theme colors (`Appearance::theme()`) — don't introduce new hex codes. Subtle visual hierarchy, not garish color blocks.

### 5. Fold long tool_results

When a tool_result's content is longer than ~10 lines OR 500 chars:
- Show the first ~3 lines + a "▶ Show full output (N lines)" affordance
- Click-to-expand (if pane interaction supports it; if not, just show first N lines + total count)

If interactive click-to-expand is hard, the minimum is: cap rendered tool_result content at ~3 lines + show "... (truncated, full result NN lines)" marker. That's good enough for this round.

### 6. Verify no regressions

- The 2 existing unit tests from Phase 3b-A still pass
- Open a MeridianAgent pane against builder-2's session (or any agent with substantial content) — verify it renders without crashing
- `cargo check -p warp_terminal -p warpui`
- `cargo check --bin warp-oss`
- `cargo clippy --bin warp-oss --all-targets`
- `cargo test` on the crate where the pane lives

## Out of scope (deferred)
- **Input from pane** (Phase 3b-C — typing into the pane sends a turn to the agent). Separate dispatch.
- **Agent registry sidebar** (Phase 3b-D — list of all spawned agents). Separate dispatch.
- **Persistence across Hyperdrive restarts** (B2's flagged follow-up — needs new SQLite table). Defer.
- **LaTeX math typesetting** — explicit non-goal this round.
- **Streaming intra-turn rendering** (showing assistant text appear character-by-character as it streams). The live refresh gives you turn-by-turn live updates from the JSONL, which is sufficient; intra-turn streaming would require integrating with stream-json output from the dispatch script. Defer.

## Files Builder will touch
- `/app/src/pane_group/pane/meridian_agent_pane.rs` (the main pane file; extend it)
- Possibly a new file `/app/src/pane_group/pane/meridian_agent_view.rs` or `/app/src/pane_group/pane/meridian_agent_markdown.rs` if the file gets too large
- Possibly `/app/Cargo.toml` if a new dep (e.g., `pulldown-cmark`, `syntect`) is needed — these are already in many Rust GUI projects and may already be transitive
- Cargo.lock if deps shift

## Reporting

Return EXACTLY one JSON object:

```json
{
  "status": "ok" or "error",
  "commit_sha": "<short SHA on wt2>",
  "files_changed": [...],
  "files_added": [...],
  "deps_added": [...],
  "live_refresh_mechanism": "<notify-debouncer-full / polling fallback / other>",
  "markdown_lib": "<pulldown-cmark / existing-warp-helper / other>",
  "code_block_highlighting": "<syntect / minimal / none>",
  "cargo_check_warp_oss": "ok" or "fail",
  "cargo_clippy_warp_oss": "ok" or "fail",
  "test_count": <total>,
  "notes": "<surprises, design choices, anything Manager should know>",
  "follow_up_for_manager": "<what's now ready for 3b-C, what new follow-ups surface>"
}
```

On error: `{"status": "error", "step": "<which>", "reason": "<root>", "files_already_modified": [...]}`.

## Notes for Builder
- **Cost is not a concern** — CTO is on Claude Max 20x. Take the time you need. Read deeply before you touch anything. Insight 01 antidote applies — this round adds 5 features at once; read the existing `meridian_agent_pane.rs` and surrounding warpui patterns thoroughly before designing.
- This round is **substantial** — 5 features, expect 100–250 turns. Use Edit/Read/Bash freely.
- **Order I'd suggest internally** (B2 picks final order): selection+scroll first (smallest, lowest risk), then markdown rendering (medium, clear scope), then visual differentiation (uses what's been built), then fold-long-results (touches the visual differentiation), then live refresh last (highest risk — async/watcher patterns are new ground in this codebase). If you hit a wall on live refresh, commit what works of the other 4 + BLOCKER → Manager on the refresh piece.
- This is the visible polish round that makes the pane actually feel like a product. Quality matters more than throughput.

# Spec — Phase 2a: Dispatch Bridge (`claude --resume --print` based)

> First end-to-end mechanic for the Manager-auto-relay workflow. Replaces the human copy-paste step between agent panes with a Manager-side shell invocation that appends turns to a Builder's session via `claude --resume <id> --print`. NO Rust crate work this round — pure shell scripting + Claude Code hooks. The Rust `meridian_manager` wiring (Phase 2b) ports this to a Rust method after the mechanic is dogfooded.

## Purpose
Eliminate the human-in-the-loop copy-paste between agent terminal panes. After Phase 2a, CTO opens Builder panes manually as before, but Manager dispatches text into Builder sessions and ingests Builder responses programmatically. CTO's primary chat is just the Manager pane.

## Mechanism (verified via Claude Code docs + on-disk inspection)

Claude Code sessions are append-only JSONL files at `~/.claude/projects/<url-encoded-cwd>/<session-uuid>.jsonl`. The CLI supports `claude --resume <uuid> --print "<text>" --output-format json` which:

- Loads the session's full history (CLAUDE.md + all prior turns) into context
- Appends the dispatch text as a new user turn to the session JSONL
- Runs Claude's response to that turn
- Appends Claude's response to the JSONL
- Exits, printing the response as JSON to stdout

Manager runs this from a shell, captures the JSON, parses out the response, and treats it as the Builder REPORT. The same `claude` binary that's used interactively handles this — no new dependencies, no daemon, no polling.

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| Where do agents register their session IDs? | `~/.meridian/agents/<role>/session-id` on local disk (not git-tracked — session IDs are ephemeral per machine) | Manager reads this to know who to dispatch to. Local-only state, one source per role. |
| How is the registry populated? | Each agent's `.claude/settings.json` adds a `SessionStart` hook that writes its session UUID to the registry on session creation | Hook fires automatically on every `claude` launch; agent doesn't have to remember to register |
| Roles: how identified? | Static role names per worktree (`builder-1`, `builder-2`, `builder-3`, `laniakea`). Each agent's hook is hardcoded to its own role string. | The role-to-worktree binding is already fixed (`wt1` = Builder 1, etc.); no need for dynamic discovery in v1 |
| Manager's own session-id | Skip in v1 — Manager dispatches, doesn't receive | Manager-to-Manager handoffs aren't a Phase 2a use case |
| `--fork-session` flag: use or skip? | **Skip** by default. Dispatch writes directly to Builder's main session so all turns appear in one canonical JSONL. | Continuity across dispatches is important; CTO can drop into Builder's pane to see the conversation; forking would create divergent histories |
| What if Builder's interactive pane is open during dispatch? | Documented but untested. Best practice: CTO doesn't type into Builder's pane while Manager is dispatching. The pane is for read-only inspection. If conflicts surface, escalate to `--fork-session` strategy or session-locking. | Research spike marked this as unverified; we accept the risk for v1 and iterate if it bites |
| Synchronous vs async dispatch | Synchronous in v1 — `claude --resume --print` blocks until Builder responds, Manager captures stdout, processes inline | Simpler model; async (Manager fires and forgets, captures later) is a Phase 2b refinement |
| Where do the scripts live? | `script/meridian-dispatch.sh` (Manager-side primitive) + `script/meridian-record-session.sh` (hook handler) — both at repo root, checked in | Discoverable, same place as `script/setup-merge-drivers.sh`; shared across all worktrees |
| JSON output format | `--output-format json` — single-object result. Manager parses with `jq`. | `stream-json` is for live streaming; we want one captured result. `text` loses structure |
| Failure modes (registry missing, Builder dead, etc.) | Script exits non-zero with a clear stderr message. Manager surfaces error to CTO; no retries. | Phase 2a is dogfood; let failures be loud and obvious so we learn the real edge cases. Retries are Phase 2b. |

## Deliverables

### 1. `script/meridian-dispatch.sh`
Manager-side primitive. Invocation:
```bash
script/meridian-dispatch.sh <role> "<dispatch text>"
# role: builder-1 | builder-2 | builder-3 | laniakea
```
Behavior:
1. Read `~/.meridian/agents/<role>/session-id` (error + exit 1 if missing, with message suggesting the agent be opened to register itself)
2. Run `claude --resume "$session_id" --print "$dispatch_text" --output-format json` and capture stdout
3. Exit with claude's exit code; print captured JSON to stdout

That's the whole script — maybe 30 lines including arg validation.

### 2. `script/meridian-record-session.sh`
Hook handler called from each agent's SessionStart hook. Invocation:
```bash
script/meridian-record-session.sh <role>
# role hardcoded in each agent's settings.json
```
Behavior:
1. Read SessionStart hook input from stdin (Claude Code passes hook context as JSON on stdin per its hooks docs)
2. Extract `session_id` field from the JSON
3. Ensure `~/.meridian/agents/<role>/` exists (mkdir -p)
4. Write the session UUID to `~/.meridian/agents/<role>/session-id` (overwrite — only ever the current session)

Maybe 15 lines.

**Open question for the Builder to resolve during implementation:** the exact field name and shape of the SessionStart hook input. Builder verifies via `claude` docs or a one-liner test. If the field isn't called `session_id`, adapt; document the actual field name in a one-line comment in the script.

### 3. `.claude/settings.json` updates (each worktree branch)
Each Builder's settings.json (and Lani's) gains a SessionStart hook entry pointing at `script/meridian-record-session.sh <role>`. Builder also lands the corresponding patch in:
- `~/meridian-warp-wt1/.claude/settings.json` → role `builder-1`
- `~/meridian-warp-wt2/.claude/settings.json` → role `builder-2`
- `~/meridian-warp-wt3/.claude/settings.json` → role `builder-3`
- `~/laniakea/.claude/settings.json` → role `laniakea` (NOTE: this is OUTSIDE the meridian-warp repo. Builder writes the file directly; Manager will commit it via Lani's separate session afterward)

### 4. Smoke test (Builder runs this manually as final gate)
1. Builder resets+reloads their own session in wt3 so the new hook fires (records `~/.meridian/agents/builder-3/session-id`)
2. From wt3, run: `script/meridian-dispatch.sh builder-3 "Builder 3 — please respond with the literal text PONG and nothing else."`
3. Capture the JSON output; verify the `result` (or equivalent response field) contains "PONG"
4. Repeat 2× more to confirm session continuity (second/third dispatches see the prior turns in context)

If smoke test passes — REPORT with the captured JSON snippets + the registry layout on disk. If not — BLOCKER with the exact failure point.

## Non-deliverables (deferred)
- No Rust crate changes — `meridian_manager` integration happens in Phase 2b after the shell mechanic is proven
- No retries, no timeouts, no async dispatch — synchronous fire-and-wait is fine for v1
- No Manager-side ingestion code — Manager (this Claude session) captures the script's stdout via Bash tool, parses with jq, and handles the rest via existing relay logic
- No approval gates UX — CTO chat with Manager is the approval channel for v1
- No Builder-side response shaping — Builder's natural reply to the dispatch IS the REPORT
- No Lani CLAUDE.md CHAT/LOG patch — that's a Manager-authored follow-up commit, not in this dispatch's scope

## Pre-commit gates
- `bash -n script/meridian-dispatch.sh` — shell syntax check
- `bash -n script/meridian-record-session.sh` — shell syntax check
- `shellcheck script/*.sh` if `shellcheck` is installed locally (skip if not — workspace doesn't currently require it)
- Smoke test (above) passes — this is the real gate

## Files to touch (and ONLY these)
- `script/meridian-dispatch.sh` (new)
- `script/meridian-record-session.sh` (new)
- `~/meridian-warp-wt3/.claude/settings.json` (in your own wt3 — register the builder-3 hook)
- `~/laniakea/.claude/settings.json` (write directly — Lani's separate repo, no commit needed from you; flag in REPORT so Manager can verify)

Settings.json changes for wt1 and wt2 — DO NOT touch those worktrees. Manager will propagate the same pattern there via cherry-pick after your wt3 work proves the mechanic.

## Notes
- Spec authored after a research spike (`claude-code-guide` agent) that ruled out 3 of 4 candidate mechanisms (hooks-autonomous-loop, stdin-multiplex, named-pipe). `--resume --print` is the only mechanism with firm documented support — see Lani's session log for the full report.
- Session UUIDs are discoverable as filenames at `~/.claude/projects/<encoded-cwd>/<UUID>.jsonl` if the registry approach somehow fails; the hook is the easy path, the directory scan is the fallback.
- `--fork-session` flag exists and could be the answer to the interactive-conflict risk; revisit if the v1 mechanic surfaces problems.
- If Builder hits any wall in implementation (hook input format, claude flag behavior, etc.) — STOP and BLOCKER → Manager. We expect 1-2 hours of Builder time max.

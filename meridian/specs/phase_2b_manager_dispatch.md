# Spec — Phase 2b: `meridian_manager::dispatch_to` (Rust port of the shell mechanic)

> Port Phase 2a's shell dispatch into the `meridian_manager` crate as a typed Rust method. Manager becomes a real orchestrator-capable Rust binary: holds the 4 leaves (Phase 1) and can now programmatically dispatch into Builder sessions (Phase 2a's mechanic, but invokable from Rust). No new transport — still shells out to `script/meridian-dispatch.sh` under the hood. The `Transport` trait abstraction stays deferred (introduce when a second transport is needed, not before).

## Purpose
Make the dispatch mechanic available to Rust callers. After this round, a Rust program can construct a `Manager` (Phase 1) and call `manager.dispatch_to("builder-1", "...")` to send a turn into a live Builder session and receive the JSON response as a structured value — no shell-out from the caller's perspective.

This is the bridge between Phase 1's library code and Phase 2a's working mechanic. After Phase 2b, the eventual binary CLI or daemon (Phase 3+) sits on top of a coherent Rust API.

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| Where does the method live? | `impl Manager { pub async fn dispatch_to(...) }` in `meridian_manager` | Manager already composes the 4 leaves; dispatch is the natural orchestration entry point |
| Method signature | `pub async fn dispatch_to(&self, role: &str, text: &str) -> Result<DispatchResponse, ManagerError>` | Async to match Phase 1's runtime; `&self` because dispatch is read-only against Manager's own state (no mutation needed) |
| `DispatchResponse` shape | Strongly-typed struct: `result: String`, `session_id: String`, `duration_ms: u64`, `total_cost_usd: f64`, `num_turns: u32`, `is_error: bool`, `permission_denials: Vec<String>` | Captures the useful fields from claude's JSON output; ignores noisier fields (model usage breakdown, etc.) |
| Transport implementation | Shell out to `script/meridian-dispatch.sh` via `command::r#async::Command` (workspace's clippy-permitted async process crate) | Phase 2a verified the shell works; no reason to reimplement the `claude --resume --print` plumbing in Rust this round |
| Script path resolution | Optional explicit path; default to `script/meridian-dispatch.sh` relative to `repo_root` (held by Manager's `WorktreeManager`) | Tests can pass a stub script; prod uses the real one. `repo_root` is already in Manager's state from Phase 1. |
| Error model | Extend `ManagerError` with `Dispatch(DispatchError)` where `DispatchError` covers: `ScriptNotFound`, `ScriptFailed { exit_code, stderr }`, `JsonParseError(serde_json::Error)`, `ClaudeReportedError { message }` | One error variant per failure mode; preserves exit codes + stderr for debugging |
| Approval gates | NOT in this round | Approval gate logic lives in `meridian_relay::RelayBus`; Phase 2c integrates the gate with dispatch (pre-dispatch approval flow). Phase 2b is just "send and receive" |
| `permission_denials` field | Capture as `Vec<String>` of tool names that were denied. Manager-level decision on what to do with them stays out of scope for this round. | Builder identity already controls behavior; this field is for diagnostics |
| Retries / timeouts | NOT in this round | Sync fire-and-wait matches Phase 2a; iterate when we hit real timeout problems |
| Test strategy | Stub script that echoes a canned JSON response based on env var. Tests pass `script_path` override to use the stub. | No live `claude` invocation in CI; faithful to the JSON envelope shape |

## Core Types

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DispatchResponse {
    pub result: String,
    pub session_id: String,
    pub duration_ms: u64,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub is_error: bool,
    #[serde(default)]
    pub permission_denials: Vec<PermissionDenial>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PermissionDenial {
    pub tool_name: String,
    // Other fields ignored; tool_name is sufficient for diagnostics
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("dispatch script not found at {0}")]
    ScriptNotFound(PathBuf),
    #[error("dispatch script failed (exit {exit_code}): {stderr}")]
    ScriptFailed { exit_code: i32, stderr: String },
    #[error("failed to parse claude JSON response: {0}")]
    JsonParseError(#[from] serde_json::Error),
    #[error("claude reported error: {message}")]
    ClaudeReportedError { message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

Extend the existing `ManagerError`:
```rust
pub enum ManagerError {
    // ...existing variants...
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
}
```

## Public API

```rust
impl Manager {
    /// Dispatch a turn into a role's existing Claude Code session.
    ///
    /// Shells out to the project's dispatch script (default
    /// `script/meridian-dispatch.sh` relative to `repo_root`).
    /// Captures the JSON envelope `claude --print --output-format json`
    /// produces and parses it into a `DispatchResponse`.
    pub async fn dispatch_to(
        &self,
        role: &str,
        text: &str,
    ) -> Result<DispatchResponse, ManagerError>;

    /// Same as `dispatch_to` but with an explicit script path.
    /// Used by tests; production callers should use `dispatch_to`.
    pub async fn dispatch_to_with_script(
        &self,
        role: &str,
        text: &str,
        script_path: impl AsRef<Path>,
    ) -> Result<DispatchResponse, ManagerError>;
}
```

The default `dispatch_to` resolves the script as `<repo_root>/script/meridian-dispatch.sh`. `repo_root` is already accessible via Manager's internal `WorktreeManager` — either expose a `repo_root()` accessor or inline the join.

## Tests (mandatory; must pass `cargo test -p meridian_manager`)

All existing Phase 1 tests must continue to pass. Add:

1. **`dispatch_to_with_stub_script_returns_parsed_response`** — write a tempdir stub script that prints canned JSON; verify `DispatchResponse` parses correctly.
2. **`dispatch_to_propagates_script_failure`** — stub script exits non-zero with stderr message; verify `DispatchError::ScriptFailed` with correct exit code + stderr.
3. **`dispatch_to_handles_invalid_json`** — stub script prints non-JSON; verify `DispatchError::JsonParseError`.
4. **`dispatch_to_handles_claude_error_envelope`** — stub script prints valid JSON with `is_error: true`; verify the response is returned (caller decides what to do) — NOT auto-promoted to a Rust error. The `is_error` flag is data, not an exception.
5. **`dispatch_to_default_script_path_resolves_relative_to_repo_root`** — construct a temp Manager with a temp `repo_root` that contains `script/meridian-dispatch.sh` (a stub); verify it gets invoked correctly without an explicit path.
6. **`dispatch_response_round_trip`** — serialize a real captured response (one from B1's verification) as a string literal, deserialize it, assert all fields land correctly. (Pin against real-world JSON shape.)

Stub script pattern for tests:
```bash
#!/bin/bash
# Prints canned JSON based on env var
echo "$DISPATCH_RESPONSE_JSON"
exit "${DISPATCH_EXIT_CODE:-0}"
```

## Cargo.toml additions

```toml
[dependencies]
# (existing)
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
# command + thiserror + tokio + the 4 leaves already present
```

`serde_json` is already a workspace dep (used by meridian_laniakea). `serde` with `derive` may also be. Builder verifies.

## Out of scope (defer to Phase 2c / later)
- Approval gates integration (relay-side already exists; dispatch-side hook is Phase 2c)
- Async fire-and-forget dispatch (Phase 2b stays synchronous)
- Streaming response (Phase 2a already passes `--output-format json` which is one-shot; streaming would use `stream-json` and a different return shape)
- Retry / timeout / circuit-breaker logic
- Multi-Builder fanout in a single call (callers can fan out themselves with `tokio::join!`)
- `Lani-side`-specific dispatch helpers (`dispatch_to("laniakea", text)` works the same way; no special path needed yet)
- Builder-pane UI refresh problem (orthogonal — Phase 3+ territory)

## Files to touch (and ONLY these)
- `crates/meridian_manager/Cargo.toml` (add serde + serde_json if not already)
- `crates/meridian_manager/src/lib.rs` (the `dispatch_to` method, `DispatchResponse`, `DispatchError`)
- `Cargo.lock` (auto-updates; commit alongside per pattern 02)

NOT touched: the leaf crates (no API changes needed), the shell scripts (Phase 2a artifacts stay as-is), any settings.json (the wire protocol via `script/meridian-dispatch.sh` is the contract).

## Pre-commit gates
- `cargo check -p meridian_manager` — typecheck
- `cargo clippy -p meridian_manager --all-targets` — lint
- `cargo test -p meridian_manager` — all tests (including the 5 mandatory new + the 5 existing from Phase 1 Round 5)

## Notes
- This is the smallest unit of work that bridges Phase 1's library code with Phase 2a's working mechanic. After it lands, future phases (DAG queue, approval gates, multi-step pipelines) have a stable Rust API to build on.
- B3 or B2 are reasonable picks. B2 wrote the original `meridian_manager` wiring in Round 5 so they have the most context; B3 just shipped Phase 2a so they have fresh context on the script. Either works; Manager will pick based on idle state.
- If during implementation the Builder discovers the script's JSON envelope has a field structure that doesn't match what's documented here, STOP and BLOCKER → Manager. Don't guess at field names.
- Cost considerations: each `dispatch_to` call invokes `claude --resume --print` which is a real API call (~$0.20-0.35 per dispatch from Phase 2a's measurements). Code that loops over `dispatch_to` is expensive. Future Phase 2c will add batching / caching primitives.

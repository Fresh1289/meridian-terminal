# Spec — meridian_agents (Phase 1, leaf crate)

> Real-logic slice. Builder TBD — dispatched in **round 4**, NOT round 3, because this crate depends on `meridian_worktree`'s first-logic slice landing first.

## Purpose
Spawn / lifecycle / role injection / kill (graceful) for managed agent processes.

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| What's an Agent? | A managed Claude Code subprocess running in a specific working dir with its own CLAUDE.md identity | Matches the v1.5.0 model + the Warp-fork-Builder pattern we just bootstrapped |
| Spawn primitive | `tokio::process::Command` — long-lived child holding stdin/stdout/stderr | Standard; allows graceful kill |
| Role injection | File-system contract: ensure correct CLAUDE.md is in the worktree before spawn. NO stdin prompt injection. | The worktree's CLAUDE.md is already the source of truth (per the per-branch identity infrastructure we built); replicating role via stdin is redundant + fragile |
| Lifecycle states | `Spawning` → `Online` → `Idle` → `Killed` | Minimal but sufficient for orchestration |
| Kill semantics | Try graceful (SIGTERM) with 5s timeout, fall back to SIGKILL | Standard pattern for managed subprocesses |
| Worktree dependency | Use `meridian_worktree::Worktree` as the working-dir source of truth | Single source for "where does this agent live" |

## Core Types

```rust
pub enum AgentState { Spawning, Online, Idle, Killed }

pub struct Agent {
    pub id: Uuid,
    pub name: String,           // "Builder 1", "Designer", etc.
    pub worktree: Worktree,     // from meridian_worktree
    pub state: AgentState,
    child: Child,               // private; tokio::process::Child
}

pub struct AgentManager {
    agents: HashMap<Uuid, Agent>,
}
```

## Public API
- `AgentManager::new() -> Self`
- `async fn spawn(&mut self, name: &str, worktree: Worktree, command: &str) -> Result<Uuid, AgentError>`
  - `command` is what to run inside the worktree (e.g. `"claude"`)
  - Verifies CLAUDE.md exists in worktree.path before spawning
- `async fn kill(&mut self, id: Uuid) -> Result<(), AgentError>` (graceful with timeout fallback)
- `fn list(&self) -> Vec<&Agent>`
- `fn get(&self, id: Uuid) -> Option<&Agent>`

## Errors
`thiserror`-derived `AgentError`: `WorktreeMissingClaudeMd(PathBuf)`, `SpawnFailed(#[from] std::io::Error)`, `UnknownAgentId(Uuid)`, `KillTimeout`, `Worktree(#[from] WorktreeError)`.

## Dependencies
```toml
tokio = { version = "*", features = ["process", "time", "rt", "macros"] }
uuid = { version = "*", features = ["v4"] }
thiserror = "*"
meridian_worktree = { path = "../meridian_worktree" }
```

## First-Slice Scope

**IN:**
- All types above
- `spawn` / `kill` / `list` / `get` impls
- CLAUDE.md presence check before spawn
- Graceful-kill with 5s timeout fallback
- 3 integration tests: spawn `sleep 60`-style stub → verify Online → kill → verify Killed; spawn missing CLAUDE.md → returns error; double-kill returns UnknownAgentId
- Tests use `meridian_worktree`'s `WorktreeManager` to create real worktrees in tempdir

**OUT (defer):**
- Stdout/stderr capture & streaming back to Manager (round 5+)
- Restart-on-crash semantics
- Per-agent resource limits (CPU/memory)
- IPC/RPC into the running agent — that's `meridian_relay`'s job

## Commit
Single atomic commit: `[Builder] Implement meridian_agents first-logic slice (Phase 1)`. Include `Cargo.toml`, `Cargo.lock`, `crates/meridian_agents/**`. Pre-commit gates mandatory.

## Round 4 Trigger
Dispatch THIS spec only AFTER `meridian_worktree`'s round-3 commit is merged to main and the workspace builds clean. Builder will resolve `meridian_worktree = { path = "../meridian_worktree" }` against the merged version.

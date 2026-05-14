# Spec — meridian_worktree (Phase 1, leaf crate)

> First-real-logic slice. Builder dispatched: B2 (wt2).

## Purpose
Wrap `git worktree` operations so the orchestrator can isolate each spawned agent in its own working directory.

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| gix vs shell-out | Shell-out to `git` via `tokio::process::Command` | Predictable, well-trodden in v1.5.0; gix adds heavy dep + steeper learning |
| Worktree naming convention | `<repo-parent>/<repo-name>-<branch>` (e.g. `~/meridian-warp-wt1`) | Mirrors the existing CTO pattern; readable; not buried in `.meridian/` |
| Branch creation | Caller passes existing branch; this crate doesn't create branches | Keeps responsibility narrow |
| Cleanup semantics | `remove()` runs `git worktree remove --force` then deletes branch refs only if `delete_branch: true` | Forced removal acceptable since orchestrator owns the worktree |

## Core Types

```rust
pub struct Worktree {
    pub branch: String,
    pub path: PathBuf,
}

pub struct WorktreeManager {
    repo_root: PathBuf,
}
```

## Public API
- `WorktreeManager::new(repo_root: impl Into<PathBuf>) -> Self`
- `async fn add(&self, branch: &str) -> Result<Worktree, WorktreeError>`
- `async fn remove(&self, worktree: &Worktree, delete_branch: bool) -> Result<(), WorktreeError>`
- `async fn list(&self) -> Result<Vec<Worktree>, WorktreeError>`

## Errors
`thiserror`-derived `WorktreeError` enum: `GitFailed { stderr: String }`, `PathExists(PathBuf)`, `BranchMissing(String)`, `Io(#[from] std::io::Error)`.

## Dependencies
```toml
tokio = { version = "*", features = ["process", "rt", "macros"] }
thiserror = "*"

[dev-dependencies]
tempfile = "*"
```

## First-Slice Scope

**IN:**
- `Worktree`, `WorktreeManager`, `WorktreeError` types
- `add` / `remove` / `list` impls invoking `git` via subprocess
- 3 integration tests using `tempfile` to spin up a real git repo: add-then-list, add-then-remove, add-twice-same-branch returns PathExists
- Tests must `git init`, set a dummy `user.email`/`user.name` in the temp repo so commits/branches work

**OUT (defer):**
- `git worktree prune`, `git worktree repair`, `git worktree lock/unlock`
- Branch creation (out of scope per Decisions table)
- Worktree-state introspection beyond branch+path

## Commit
Single atomic commit: `[Builder] Implement meridian_worktree first-logic slice (Phase 1)`. Include `Cargo.toml`, `Cargo.lock`, `crates/meridian_worktree/**`. Pre-commit gates mandatory.

# Spec — meridian_manager (Phase 1 Round 5, wiring crate)

> Wires the 4 leaf crates (`meridian_relay`, `meridian_worktree`, `meridian_agents`, `meridian_laniakea`) into a coherent `Manager` orchestrator. Replaces the current 1-test scaffold. **No new domain logic** — composition only.

## Purpose
Provide the orchestration entry point that holds all 4 leaves and exposes a minimal coherent API. This is the Phase 1 "first-logic slice" of the Manager: enough to prove the 4 leaves compose cleanly under one type, with end-to-end tests that exercise the composition (not re-test each leaf).

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| Single struct or trait family? | Single concrete `Manager` struct | Phase 1 YAGNI — no second implementation in sight |
| Identity exposure | Replace `manager_identity()` fn with `pub const MANAGER_IDENTITY: &str = "Meridian Manager"` | Constant matches usage; function was a placeholder |
| Construction | `Manager::new(repo_root, knowledge_root).await` — both paths required to exist | KnowledgeStore::load already errors on missing dir; repo_root is needed for worktree manager |
| Error model | One `ManagerError` enum that flattens the four leaf error types via `#[from]` | Standard thiserror pattern; mirrors how `meridian_agents` re-exports `WorktreeError` |
| Receiver ownership | `Manager` owns `RelayReceiver` directly; expose `&RelaySender` + `&mut self` `recv_relay()` | Single-consumer pattern is the right default; matches the v1.5.0 Manager-as-bus convention |
| Shutdown | `shutdown(&mut self)` walks tracked agents and kills each via `AgentManager::kill` | Don't reach into `child` directly — use the leaf's already-tested kill path |
| Knowledge access | Read-only accessor `&KnowledgeStore` for now; no Manager-driven appends in Phase 1 | Manager queries Laniakea for context; appends are Laniakea's CLI agent today |
| Worktree access | Expose `&WorktreeManager` (immutable methods are all `&self` already) | `WorktreeManager` is internally cheap to call repeatedly |
| Agents access | Expose `&mut AgentManager` accessor | Spawn/kill require `&mut self` on the leaf |
| Async runtime | Stay on tokio (matches every leaf) | Already the workspace standard |

## Core Types

```rust
pub const MANAGER_IDENTITY: &str = "Meridian Manager";

pub struct Manager {
    relay_sender: RelaySender,
    relay_receiver: RelayReceiver,
    worktrees: WorktreeManager,
    agents: AgentManager,
    knowledge: KnowledgeStore,
}

#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error(transparent)]
    Relay(#[from] RelayError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    Agent(#[from] AgentError),
    #[error(transparent)]
    Knowledge(#[from] KnowledgeError),
}
```

## Public API

```rust
impl Manager {
    /// Construct the orchestrator. Both directories must already exist.
    pub async fn new(
        repo_root: impl Into<PathBuf>,
        knowledge_root: impl AsRef<Path>,
    ) -> Result<Self, ManagerError>;

    /// Send relays to Builders.
    pub fn relay_sender(&self) -> &RelaySender;

    /// Receive the next relay routed back to Manager. Returns `None` when
    /// the bus channel has closed.
    pub async fn recv_relay(&mut self) -> Option<Relay>;

    /// Read-only worktree-manager handle.
    pub fn worktrees(&self) -> &WorktreeManager;

    /// Mutable agent-manager handle (spawn/kill require `&mut`).
    pub fn agents_mut(&mut self) -> &mut AgentManager;

    /// Read-only knowledge-store handle for context queries.
    pub fn knowledge(&self) -> &KnowledgeStore;

    /// Gracefully kill every tracked agent. Returns the first error
    /// encountered; remaining agents are still kill-attempted.
    pub async fn shutdown(&mut self) -> Result<(), ManagerError>;
}
```

## Tests (mandatory; must all pass `cargo test -p meridian_manager`)

1. **`manager_identity_is_stable`** — keep the existing scaffold test, retargeted to `MANAGER_IDENTITY` constant.
2. **`manager_constructs_from_temp_environment`** — `tempfile::TempDir` for both repo_root and knowledge_root (knowledge_root just needs to exist; can be empty). Run `git init` in repo_root to make WorktreeManager methods usable downstream. Assert `Manager::new(...)` returns `Ok` and that `manager.knowledge().query(&Query::default())` returns an empty Vec.
3. **`manager_routes_relay_roundtrip`** — Construct Manager, send a non-approval relay via `manager.relay_sender().send(...)`, then `manager.recv_relay().await` and assert id round-trips.
4. **`manager_shutdown_with_no_agents_is_clean`** — Construct Manager, call `shutdown()` with no agents tracked, assert `Ok(())`.
5. **`manager_knowledge_query_passthrough`** — Pre-write one `decisions.jsonl` entry into the temp knowledge_root before `Manager::new`, construct, query, assert one result returned.

Use `tempfile` (already a workspace dev-dep, used by `meridian_agents` + `meridian_laniakea`) for filesystem fixtures.

## Cargo.toml — full dependency list

Replace the empty `[dependencies]` block in `crates/meridian_manager/Cargo.toml` with:

```toml
[dependencies]
meridian_agents = { workspace = true }
meridian_laniakea = { workspace = true }
meridian_relay = { workspace = true }
meridian_worktree = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt", "sync"] }

[dev-dependencies]
tempfile = { workspace = true }
```

**No workspace.dependencies changes** — every dep already listed in the workspace root. This is the first round where the alphabetical-slot merge conflict (decision 09) cannot fire.

## Out of scope (defer to later phases / rounds)
- Actual Manager system-prompt content (`MANAGER_IDENTITY` is a placeholder; the real persona prompt comes when meridian_manager wires to a Claude API call in Phase 2)
- Manager-driven knowledge appends (Laniakea's CLI agent still owns writes in Phase 1)
- Approval-gate flows in tests (the leaf already covers them; Manager just exposes the handle)
- Real `spawn` integration test at Manager level (the leaf covers it; staging a spawnable CLAUDE.md + stub binary at Manager level duplicates that surface)
- Persona swap / context rotation / DAG queue — Phase 2 territory

## Notes
- This spec was authored AFTER reading each leaf's `lib.rs` on main directly. Public-API call signatures pinned to what actually exists, not what was assumed (insight 01 antidote — third application, after Round 4 cherry-pick and the vault-reorg survey).
- Cargo.lock will pick up new dependency edges; commit it alongside (per pattern 02).
- Builder may discover that `KnowledgeStore::load` requires the dir to exist BEFORE construction (caller's responsibility) — that's intentional, not a bug.
- If Manager finds the leaf signatures changed subtly under their feet during implementation (unlikely — main is frozen), STOP and BLOCKER → Manager.

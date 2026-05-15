# MERIDIAN — Transfer Plan

> This repository is **the next Meridian**: a fork of OpenWarp (`zerx-lab/warp`), itself a fork of `warpdotdev/warp`. The goal is to transfer the full Meridian feature set — minus the topology canvas — into a Rust foundation that ships as a native terminal-first product.
>
> The **Meridian brand** is alive. The previous Meridian (v1.5.0 Electron app, `Fresh1289/meridian`) is frozen, not retired. This is the platform shift, not a rebrand.
>
> **Topology canvas is explicitly deferred.** Everything else from the old app gets ported. Canvas comes back as its own phase once the core is solid.

---

## North Star

A native, Rust-based, AI-coding-agent orchestrator that:

- Routes multi-agent work through a **Manager** persona (canonical Meridian-app pattern)
- Spawns and isolates parallel **Builder** agents (worktree-based or equivalent)
- Carries **Laniakea** — the institutional memory / Mentat — as the always-on knowledge layer
- Is **multi-provider** (Claude Code, Codex, Gemini CLI, any future CLI agent)
- Treats the **terminal as the substrate**, not a tab — Warp's foundation makes this real
- Keeps **Obsidian vaults** as the source of truth for cross-session knowledge

### The CTO Interface (clarified 2026-05-14)

The Warp-fork is **not** a full-autonomy orchestrator like the v1.5.0 Electron app was. The vision is more surgical:

- **Manual spawn stays.** CTO opens each agent in its own terminal pane (worktree + `claude` CLI per agent), keeping that direct pane affordance always available. Builders/Designers/Lani are real, separately-accessible Claude Code sessions.
- **The relay is what gets automated.** Today CTO copy-pastes between panes (`Manager → wt1 terminal`, then `wt1 REPORT → Manager terminal`). Tomorrow Manager does that transmission programmatically. The dispatch *content* is identical — only the carrier changes.
- **CTO's primary chat is just Manager.** Manager aggregates and summarizes everything across spawned agents. CTO can always drop into any agent pane directly when they want to, but the default UX is "talk only to Manager."

This differs from v1.5.0 because the Electron app *was* the chat interface AND the orchestrator simultaneously — agents were UI rows, never visible terminals. The Warp-fork keeps the pane reality (because Warp is a terminal) and bolts auto-relay on top.

The architectural implication is that Phase 2 needs a **transport bridge**: Phase 1's `meridian_relay` is in-process mpsc; cross-pane delivery requires either a Warp-native pane API, a per-agent file-mailbox, or a per-agent IPC socket. Design discussion in `manager-state.md` under "Project Vision."

## What's Already in the Fork (Don't Re-Port)

Inherited from Warp + OpenWarp — adapt, don't rebuild:

| Capability | Source |
|---|---|
| GPU-accelerated terminal | upstream `warpdotdev/warp` |
| Command palette (Cmd+K) | upstream |
| Workflows / command signatures | upstream (`command-signatures-v2/`) |
| Themes & settings | upstream |
| Embedded shell, PTY, blocks | upstream |
| Git integration (block-level) | upstream |
| BYO AI providers, local credentials | **OpenWarp patches** |
| Telemetry decoupling (Phase 4) | **OpenWarp** in-progress |
| Oz server → local stubs (Phase 5) | **OpenWarp** in-progress |
| CLAUDE.md rule-file recognition | OpenWarp (commit `72d37d1c`) |

## What Transfers from the Old Meridian App

Grouped by domain. `[P1]` = phase 1 must-have, `[P2]` = phase 2, `[P3]` = post-canvas.

### Agent orchestration `[P1]`
- **Manager persona** — hardcoded system prompt, never user-overridable (per existing memory)
- **Agent spawning** — lifecycle: spawn → role inject → work → report → cleanup
- **Multi-agent pipeline** — Manager → (Designer →) Builder → QA → merge loop
- **Per-agent API keys + model selection** — Claude Code, Codex, Gemini CLI, any CLI tool
- **Relay routing** — Manager-as-bus pattern; agents talk through Manager, never peer-to-peer
- **Relay approval gates** — human-in-the-loop for high-stakes routing
- Source: `electron/agentManager.ts`, `electron/chatHandler.ts`, `electron/systemPrompt.ts`, `electron/supervisor.ts`, `agent-roles/*.md`

### Isolation & state `[P1]`
- **Worktree-per-agent** — port the pattern; explore container-based isolation as alt
- **Per-project config** — each project has its own agent team, prompts, knowledge
- **Auto-commit + revert** — every edit committed, one-shot revert available
- **Context rotation** — token budgeting, mid-work snapshot/swap (`electron/contextRotation.ts`)
- **Manager swap** — runtime replacement of the Manager identity (`electron/managerSwap.ts`)
- Source: `electron/worktree.ts`, `electron/projectManager.ts`, `electron/gitManager.ts`, `electron/database.ts`

### Laniakea (the Mentat) `[P1]`
- **Five-store knowledge engine** — decisions, patterns, failures, preferences, insights (JSONL)
- **Pattern detection** — failure repeats, correction → preference, loop detection, drift detection
- **Wisdom API** — consult / teach / brief / stats directives
- **Ethos injection** — per-project principles injected into every agent system prompt
- **Spawn briefing** — agents receive Laniakea context on creation
- Existing standalone scaffold at `~/laniakea/` is the seed; embed or run as service.
- Source: `electron/scribe.ts`, `specs/scribe-mentat.md`, `~/laniakea/CLAUDE.md`

### Reliability surface (R1-R9 from v1.5.0) `[P1]`
- Universal action logger (JSONL, 27 integration points)
- Hash-based loop detection + auto-restart (max 2x)
- Atomic relay processing + exclusive merge locks
- Bounded collections (500 activity, 50 cache, 1MB buffer)
- Graceful agent kill (SIGINT → 3s → SIGTERM)
- Labeled stashes + merge failure notifications
- Source: `electron/*` plus `state.md` "What shipped in v1.5.0" section

### Task queue & self-healing `[P2]`
- DAG-based task queue (`electron/dagQueue.ts`)
- Parallel execution with dependencies
- Self-heal on failure, retry with context
- Spec: `specs/task-queue-immortal.md`, `specs/immortal-meridian.md`

### Pipeline UI (without canvas) `[P2]`
- **Sidebar agent list** (replaces canvas for phase 2) — agents as rows, status badges, click-to-inspect
- **Chat panel** — Manager conversation, embedded in a Warp pane
- **Agent detail panel** — config, recent activity, model, costs
- **Relay/merge approval inline** — modals or block-level inline approvals
- **Files panel** — project file browser, edit-on-click
- Source: `src/components/AgentSpawner.tsx`, `src/components/ChatPanel.tsx`, `src/components/RelayApprovalPanel.tsx`, `src/components/MergeApprovalPanel.tsx`

### Topology canvas `[P3 — DEFERRED]`
**Out of scope for this transfer.** Comes back as a dedicated phase once P1+P2 land. Specs preserved: `specs/round3-4-ui-overhaul.md`, `specs/round2-panels-redesign.md`, `specs/meridian-v1-mockup.md`. When it returns, it may not be Canvas 2D — could be native Rust GPU rendering since Warp already has that stack.

### Misc to skip in P1
- Supabase auth (the fork is local-first; cloud sync is post-MVP)
- Onboarding redesign (Warp has workflows; defer)
- Templates panel, drive, self-update, audit log → `[P3]`
- Designer + QA agent roles → fold into general Builder roster initially; revisit once core works

## Architecture Direction

### Licensing: AGPL containment
- The Warp-fork shell is **AGPL v3** (inherited from upstream)
- The Meridian orchestration brain (Manager, agent routing, Laniakea, task DAG) can be:
  - **Option A:** also AGPL, embedded in the Rust binary — simplest, all-open
  - **Option B:** separately licensed, run as IPC/HTTP service that the shell talks to — preserves closed-source future
- Decision pending. Default to A unless a commercial path requires B.

### Where Laniakea lives
- Phase 1: standalone CLI agent (already exists at `~/laniakea/`); orchestrator calls it via subprocess
- Phase 2: embedded as a Rust crate inside the fork
- The JSONL knowledge format is the contract — both forms must read/write the same files

### Agent execution
- Inherit Warp's existing CLI-agent integration (Claude Code, Codex, Gemini CLI plug in as first-class)
- Add Meridian's Manager layer on top — Warp routes to a single agent today; we route through Manager to a fleet
- Worktree isolation handled by a new Rust crate (`crates/meridian-worktree`?) calling out to `git worktree`

### State storage
- Replace v1.5.0 SQLite with simpler files where possible (JSONL for knowledge, TOML for config)
- If a relational store is needed, prefer SQLite via `rusqlite` (no external deps)
- Project state lives in `.meridian/` under the user's project dir (same convention as v1.5.0)

## Phasing

### Phase 0 — Foundation (current)

**Per-clone bootstrap (do once on every fresh clone — these are NOT optional):**
- [x] Install Rust toolchain via rustup with `--default-toolchain` matching `rust-toolchain.toml` (currently 1.92.0). Add components `clippy` and `rustfmt`. Skipping this blocks pre-commit gates.
- [x] Run `bash script/setup-merge-drivers.sh` from the repo root. This registers the `openwarp-ours` merge driver and enables `rerere`. Without it, every `merge=openwarp-ours` rule in `.gitattributes` is a silent no-op — protected paths (warp_ssh_manager, agent_sdk, blocklist, per-branch CLAUDE.md, etc.) will get clobbered on the next merge from upstream. This was missed during initial fork bootstrap on the MacBook (caught + fixed 2026-05-14 — see Laniakea failure 03-openwarp-ours-driver-never-registered).

**Foundation deliverables:**
- [x] Fork OpenWarp → `Fresh1289/meridian-terminal`
- [x] Local clone at `~/meridian-warp` with `origin`, `openwarp`, `warp-upstream` remotes
- [x] Laniakea standalone scaffolded at `~/laniakea/`
- [x] Per-branch CLAUDE.md identity infrastructure for wt1/wt2/wt3 + merge=openwarp-ours protection on all 4 branches (2026-05-14)
- [x] Session-log discipline ported from v1.5.0 → `~/meridian-warp/session-log.md` (2026-05-14)
- [ ] Strip Warp brand assets (fonts, logo, wordmark) — do not redistribute
- [ ] Map the upstream crate workspace; identify where Meridian crates plug in
- [ ] Read `AGENTS.md`, `WARP.md`, `CONTRIBUTING.md` to internalize upstream conventions

### Phase 1 — Core orchestration
- [ ] `meridian-manager` crate — Manager persona + system prompt + routing
- [ ] `meridian-agents` crate — spawn / lifecycle / role injection / kill (graceful)
- [ ] `meridian-worktree` crate — git worktree isolation per agent
- [ ] `meridian-laniakea` crate — knowledge engine, pattern detection, wisdom API
- [ ] `meridian-relay` crate — atomic relay processing, approval gates
- [ ] Sidebar UI (no canvas): agent list, chat, approval inline
- [ ] R1-R9 reliability surface ported

### Phase 2 — Pipeline & DAG
- [x] **Transport bridge — Phase 2a (shell-based mechanic)** ✅ shipped 2026-05-14. `script/meridian-dispatch.sh` + `script/meridian-record-session.sh` + per-worktree `SessionStart` hooks. Manager dispatches via `claude --resume <id> --print --output-format json --dangerously-skip-permissions`; session UUID + cwd auto-registered by hook at `~/.meridian/agents/<role>/`. Verified live: Manager → Builder 1 round-trip in ~10s, $0.21 per dispatch. Replaces the copy-paste relay step. NO Rust crate changes this round.
- [x] **`.app` build + bundle** ✅ shipped 2026-05-15. `cargo build --release --bin warp-oss` + `cargo bundle --release --bin warp-oss` → `target/release/bundle/osx/OpenWarp.app`. Daily-driver dogfood unlocked. Spec'd at top of this file under "The CTO Interface."
- [ ] **Transport bridge — Phase 2b (Rust integration)** — *deferred, not blocking Phase 3.* Port the shell mechanic into `meridian_manager` as `Manager::dispatch_to(role, text) -> Result<Relay, ManagerError>`. Shell-out under the hood; future evolution to a `Transport` trait once a second transport (Warp-native pane API, IPC socket) is needed. Phase 3 UI can call the script directly until Phase 2b lands.
- [ ] Task DAG queue + self-heal
- [ ] Multi-step pipeline (Manager → Builder → QA loop)
- [ ] Cost tracking / token budgets / model swap
- [ ] Per-project config & ethos files
- [ ] Migration tooling for v1.5.0 users

### Phase 3 — Visible Meridian UI

Phase 3 is where the fork stops looking like upstream OpenWarp and starts looking like Meridian. This is multiple sub-phases.

**Phase 3a — Brand strip (Phase 0 carry-over; gate before any UI add)**
- [ ] Identify all Warp brand surfaces in the repo (wordmark in app name, icon files in `channels/`, copyright strings, "Warp"/"OpenWarp" string literals in user-facing strings)
- [ ] Decide Meridian visual identity (wordmark, icon, primary color, font choice — CTO direction needed)
- [ ] Strip Warp brand, add Meridian brand. Bundle identifier shifts from `dev.openwarp.OpenWarp` to something like `dev.meridian.Meridian`
- [ ] Verify .app still builds + launches with new brand

**Phase 3b — Manager pane / orchestration surface**
- [ ] Find or add a "left rail" / sidebar in Warp's UI for agent list (recon in progress)
- [ ] Manager chat as a Warp pane (or embedded chat panel) — replaces the "open `claude` in a pane" workflow with native chat
- [ ] Wire `meridian_*` crates into the binary (or call `script/meridian-dispatch.sh` from UI code)
- [ ] Live indicator of dispatched relays in agent panes (visible relay — addresses the "backend only" complaint)
- [ ] Approval gate UI inline (when Manager flags a route as high-stakes)

**Phase 3c — Canvas resurrection (deferred to its own dedicated phase)**
- [ ] Topology canvas (native Rust GPU rendering, not Canvas 2D — Warp already has GPU stack)
- [ ] Specs preserved at `~/meridian/specs/round3-4-ui-overhaul.md`, etc.

**Phase 3d — Misc polish**
- [ ] Templates, drive, self-update, audit log
- [ ] Designer + QA agent roles (re-introduce)
- [ ] Public launch readiness

## Potential Addons

External projects to evaluate for integration, inspiration, or competitive read. Not committed — parked here so they don't get lost.

- **OpenSwarm** (VRSEN/OpenSwarm, open-sourced 2026-05-12) — terminal-based multi-agent orchestrator, 8 specialized agents (research, slides, data, video, image, docs, etc.), "Claude code for everything except coding." Worth studying for: agent-routing patterns, CLI-only UX (matches our P1 sidebar-only direction), how they slice specialist roles vs. our Manager/Builder/QA. Read before locking the `meridian-manager` crate API. https://github.com/VRSEN/OpenSwarm

## Open Questions

1. **AGPL vs containment** — answer before any commercial conversation
2. **Brand assets** — design new Meridian wordmark for the Rust UI; can lift from existing `~/huang-design/`
3. **Worktree alternative** — does the inherited Warp PTY model support per-agent shell envs cleanly, or do we need real git worktrees?
4. **What to do with old app users** — the v1.5.0 DMG is on GitHub releases. Keep it up? Add migration banner?
5. **Public showcase repo** — `Fresh1289/meridian` (current showcase) — leave as v1.5.0 archive, or repurpose as marketing for the next Meridian?

## Source Material

All specs preserved at `~/meridian/specs/` in the old app repo. Frozen, not deleted. Key reading:

- `meridian-full-spec.md` — original product spec
- `scribe-mentat.md` — Laniakea/Scribe deep dive
- `agent-spawner.md`, `agent-worktrees.md` — isolation patterns
- `reliability-pass2.md`, `round1-safety-gates.md` — R1-R9 ground truth
- `immortal-meridian.md`, `task-queue-immortal.md` — DAG / self-heal
- `round3-full-pipeline.md` — pipeline mechanics

Laniakea knowledge (decisions, preferences) at `~/laniakea/knowledge/`.

---

*Authored 2026-05-11. This document evolves as the transfer progresses.*

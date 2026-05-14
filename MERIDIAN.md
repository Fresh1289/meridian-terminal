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
- [x] Fork OpenWarp → `Fresh1289/meridian-terminal`
- [x] Local clone at `~/meridian-warp` with `origin`, `openwarp`, `warp-upstream` remotes
- [x] Laniakea standalone scaffolded at `~/laniakea/`
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
- [ ] Task DAG queue + self-heal
- [ ] Multi-step pipeline (Manager → Builder → QA loop)
- [ ] Cost tracking / token budgets / model swap
- [ ] Per-project config & ethos files
- [ ] Migration tooling for v1.5.0 users

### Phase 3 — Canvas resurrection + polish
- [ ] Topology canvas (native Rust GPU rendering, not Canvas 2D)
- [ ] Templates, drive, self-update, audit log
- [ ] Designer + QA agent roles (re-introduce)
- [ ] Public launch

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

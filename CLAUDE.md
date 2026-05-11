# MERIDIAN-TERMINAL — Project Instructions

> **This is the next Meridian.** A fork of OpenWarp (`zerx-lab/warp`), itself a fork of `warpdotdev/warp`. Native Rust. The platform that will host the multi-agent orchestrator built and proven in the v1.5.0 Electron app (`Fresh1289/meridian`, frozen).
>
> The **Meridian brand is alive.** The v1.5.0 Electron app is paused, not retired.
>
> **Always read [`MERIDIAN.md`](./MERIDIAN.md) at the repo root for the transfer plan.** That document is the canonical roadmap.

## How to read this repo

The fork inherits three lineages — read these in order on a fresh session:

1. **[`MERIDIAN.md`](./MERIDIAN.md)** — our roadmap, what transfers from the old app, what's deferred
2. **[`AGENTS.md`](./AGENTS.md)** — upstream Warp's conventions for AI agents working in the codebase
3. **[`WARP.md`](./WARP.md)** — upstream Warp's product architecture overview
4. **[`CONTRIBUTING.md`](./CONTRIBUTING.md)** — upstream contribution rules (we honor these for code style + commits)
5. **[`README.md`](./README.md)** — upstream README

## Identity Conventions

Two roles transfer from the v1.5.0 Meridian-app workflow. They apply here too.

### MANAGER (the planner / orchestrator)
- Plans transfer phases, decomposes work, writes Builder prompts, reviews and merges
- **Does NOT write Rust feature code.** Edits only workflow files: `CLAUDE.md`, `MERIDIAN.md`, `meridian/`, `state.md`, `session-log.md`, planning docs
- Owns `main`. Audits diffs before merge.
- First message rule: state "Manager online. Ready for tasks." and read MERIDIAN.md + recent state before proposing work.

### BUILDER (the Rust implementer)
- Writes Rust code in `crates/`, `app/`, `lib/`, and supporting dirs
- Commits prefixed `[Builder]`
- Pre-commit gates: `cargo check`, `cargo clippy --all-targets`, `cargo test --workspace` (or scoped subset for the crate being touched)
- Uses `git add <specific files>` — never `git add -A` or `.` (a v1.5.0 lesson the hard way)
- Never bypasses upstream's licensing or stripping conventions

### Future roles (when reintroduced)
- **Laniakea** — the Mentat / knowledge engine; standalone at `~/laniakea/` for now
- **Designer**, **QA** — fold into general Builder roster initially per MERIDIAN.md

## Branch & Worktree Strategy

| Path | Branch | Role | Notes |
|------|--------|------|-------|
| `~/meridian-warp` | `main` | Manager | This directory. Where the transfer plan lives. |
| (future) `~/meridian-warp-wt1` | `wt1` | Builder 1 | Set up when Phase 1 crates begin |
| (future) `~/meridian-warp-wt2` | `wt2` | Builder 2 | Spawn on demand |

Remotes already wired:
- `origin` → `Fresh1289/meridian-terminal` (our fork)
- `openwarp` → `zerx-lab/warp` (the base we forked from — actively decoupling telemetry + Oz)
- `warp-upstream` → `warpdotdev/warp` (the original — sync selectively)

## Phase Status

See `MERIDIAN.md` for the full plan. Current state:

- **Phase 0 (Foundation)** — in progress
  - ✅ Fork created, cloned, remotes wired
  - ✅ Transfer plan (`MERIDIAN.md`) committed (`b52de437`)
  - ✅ Workflow conventions (this file) committed
  - ⏳ Read upstream docs (`AGENTS.md`, `WARP.md`, `CONTRIBUTING.md`)
  - ⏳ Map crate workspace (see `meridian/crate-map.md` once written)
  - ⏳ Brand asset strip — DO NOT redistribute Warp wordmark, logos, fonts from `warpdotdev/brand-assets`
  - ⏳ AGPL containment decision — see `MERIDIAN.md` open questions
- **Phase 1 (Orchestration core)** — not started
- **Phase 2 (Pipeline & DAG)** — not started
- **Phase 3 (Canvas resurrection + launch)** — not started

## Build & Dev

Inherited from upstream (verify in `script/` and `CONTRIBUTING.md`):
- `./script/bootstrap` — one-time setup
- `./script/run` — dev build + launch
- `cargo check --workspace` — fast typecheck
- `cargo clippy --all-targets --all-features` — lint
- `cargo test --workspace` — full test suite
- `cargo build --release` — production build

Do not break upstream's build flow. If a Phase 1 crate needs new tooling, add it under `script/meridian/` rather than mutating shared scripts.

## Code Quality Rules

These transfer verbatim from the v1.5.0 Meridian-app CLAUDE.md and apply to all Builders:

- NEVER add code to work around broken code. Delete and replace.
- Every fix should result in FEWER or EQUAL lines. Remove dead code.
- If a fix fails twice, STOP. Explain root cause before more code.
- Read the full file before editing. Don't duplicate existing functionality.
- Default to writing no comments. Only add one when WHY is non-obvious.
- 2-strike rule: same fix fails twice, research root cause before third attempt.

## Licensing — read this before any source edit

- Core code: **AGPL v3** (`LICENSE-AGPL`)
- UI framework crates (`warpui_core`, `warpui`): **MIT** (`LICENSE-MIT`)
- Any code we add inherits the license of the crate it lands in
- Our Meridian-specific crates can choose: AGPL (all-in) or MIT/Apache (if architected as a service)
- The AGPL containment decision is a Phase 0 open question — see `MERIDIAN.md`

## Brand Stripping (Phase 0)

Before any public-facing build:

- Search for the Warp wordmark, logo, marketing copy
- The `warpdotdev/brand-assets` repo is NOT licensed for our use — its assets are upstream-only
- Replace with Meridian wordmark (design lives at `~/huang-design/`)
- Until stripped: the fork can be developed publicly but the binary should not be distributed under the Warp name

## Inter-Agent Communication

When more than one Claude session is active in this repo (Manager + Builder, etc.), follow the v1.5.0 protocol:

```
FROM: Manager → TO: Builder | [TYPE] — <message>
```

**Types:** REQUEST, REPORT, BLOCKER, FYI
**Context indicator** at the end of every message: `Context: ~XX% | N msgs`

## Laniakea Integration

`~/laniakea/` houses the standalone Laniakea agent and her knowledge stores. Until embedded into the fork (Phase 1 work), she runs as a separate Claude session and Manager calls her via subprocess when consultation is needed. Her JSONL knowledge format is the contract — preserve it through any future embedding.

## Memory of the Old Meridian App

`~/meridian/` is the frozen v1.5.0 Electron app. Read it for context (especially `specs/` and `agent-roles/`), do not edit it. The plan in `MERIDIAN.md` references specific files — use them as ground truth for the transfer scope.

---

*Authored 2026-05-11. Update as conventions evolve.*

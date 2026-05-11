# Crate Map — Inherited Workspace

> Snapshot of the 63 upstream crates and where Meridian's Phase 1 work plugs in.
> Maintained by Manager. Update when new crates are added or upstream renames.

## Workspace facts

- **63 crates** in `crates/` plus `app/`
- Workspace license: `AGPL-3.0-only` (default-license; MIT-licensed UI crates declare otherwise)
- Members: `crates/*` + `app`
- Resolver: 2
- `default-members` (faster local builds): `app`, `channel_versions`, `command`, `editor`, `graphql`, `markdown_parser`, `sum_tree`, `warpui`, `warp_completer`, `warp_terminal`, `warp_util`

## What each crate is for (best-effort categorization — verify before relying)

### Core terminal & UI (use as-is)
- `app/` — the binary entry point
- `warp_terminal` (referenced in default-members) — terminal core
- `warpui` / `warpui_core` (MIT) — UI framework
- `editor` — code editor surface
- `markdown_parser`, `syntax_tree` — rendering pipelines

### AI & agent layer (this is where Meridian extends)
- `ai/` — AI provider integration (OpenWarp's BYO-provider patches landed here)
- `computer_use/` — likely agent computer-use surface
- `input_classifier/`, `natural_language_detection/` — input handling
- `command/`, `command-signatures-v2/` — command/workflow handling

### Infrastructure (likely useful for Meridian crates)
- `ipc/` — inter-process communication (useful for Manager↔Builder if subprocess model)
- `isolation_platform/` — sandboxing (relates to worktree-style isolation)
- `managed_secrets/`, `managed_secrets_wasm/` — secret/API key storage
- `persistence/` — state storage
- `jsonrpc/` — RPC plumbing
- `http_client/`, `http_server/` — HTTP layers
- `firebase/`, `graphql/` — cloud back-ends (likely strip or stub per OpenWarp's Phase 5)

### Likely-skip in Phase 1
- `onboarding/` — Warp's onboarding flow
- `remote_server/` — cloud collab
- `integration/` — test harness only
- `serve-wasm/` — wasm helper

### Misc / utility
- `simple_logger/`, `repo_metadata/`, `fuzzy_match/`, `sum_tree/`, `string-offset/`, `field_mask/`
- `asset_cache/`, `asset_macro/` — asset pipeline (brand strip needs to walk these)
- `app-installation-detection/`, `prevent_sleep/`, `channel_versions/`
- `handlebars/`, `languages/`, `node_runtime/`
- `settings/`, `settings_value/`, `settings_value_derive/`

## Where Meridian Phase 1 crates plug in

New crates to add under `crates/`:

| Crate | Purpose | Likely deps |
|---|---|---|
| `meridian_manager` | Manager persona + system prompt + routing | `ai`, `ipc`, `persistence` |
| `meridian_agents` | Agent lifecycle (spawn / kill / report) | `ai`, `ipc`, `managed_secrets`, `meridian_worktree` |
| `meridian_worktree` | Git worktree isolation per agent | `isolation_platform`, std `Command` |
| `meridian_laniakea` | Knowledge engine, pattern detection | `persistence`, `markdown_parser` |
| `meridian_relay` | Atomic relay processing + approval gates | `ipc`, `meridian_manager`, `meridian_agents` |

Each gets a `Cargo.toml` declaring AGPL-3.0-only (matching workspace), a `lib.rs` stub, and a workspace member entry.

## Open questions about the workspace

1. **Where does the agent runtime currently live?** OpenWarp wired Claude Code / Codex / Gemini CLI as first-class — find which crate hosts that integration. Likely `ai/` or `computer_use/`. Confirm before designing `meridian_agents`.
2. **Does upstream already model "multiple AI sessions"?** Read `ai/` for how concurrent AI calls are handled. Meridian's Manager-as-bus pattern may extend or replace that.
3. **Settings shape** — do we add Meridian settings to upstream's `settings/` crate, or namespace them separately? Probably the former for UX consistency.
4. **Persistence file location** — upstream's `persistence/` likely owns disk paths. Meridian's `.meridian/` per-project convention needs to integrate cleanly.

These questions are first reading targets for the Phase 1 kickoff session.

---

*Authored 2026-05-11. Update via Manager dispatches as the workspace evolves.*

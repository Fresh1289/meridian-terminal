# Phase 3 Plan — Visible Meridian UI

> Planning doc, not yet a dispatchable spec. Concrete file paths sourced from the recon pass dated 2026-05-15. Builder dispatch specs come later, derived from sub-sections of this plan.

## Plug-points discovered in the recon
The fork's UI is pane-based with a `wgpu` rendering stack and `winit` window/event abstraction. Every visible Meridian feature plugs in as either a new pane type or an extension of an existing pattern. Specifically:

| Surface | File / Location | Pattern |
|---|---|---|
| Pane type registry | `/app/src/pane_group/pane/mod.rs` (`IPaneType` enum, lines ~138–150) | Add a new variant + implement `BackingView` + `PaneContent` |
| Pane composition | `/app/src/pane_group/mod.rs` + `/app/src/pane_group/tree.rs` | `LeafContents` enum registration |
| AI chat reference | `/app/src/ai/ai_document_view.rs` | Existing AI pane; model to follow for Manager chat |
| Block model | `/app/src/terminal/model/block/serialized_block.rs` (lines 22–41 for `AgentViewVisibility`) | Extendable for relay traffic blocks |
| Brand assets | `/app/assets/bundled/{svg,png,fonts}/` (366 SVGs incl. `warp.svg`, `oz.svg`) | Asset replacement + theme color update |
| Theme system | `/app/src/themes/theme.rs` + `/app/src/themes/` subpages | Update palette + brand colors |
| Settings UI | `/app/src/settings_view/mod.rs` + subpage files | Add `meridian_page.rs` subpage |
| Channel definition | `/crates/warp_core/src/channel/mod.rs` (`Oss` variant; `warp-oss` CLI name) | Extend `Oss` OR add `Meridian` variant |
| Brand strings | Various — `/crates/warp_core/src/channel/config.rs` for constants | Audit + replace |
| Drive panel (sidebar pattern reference) | `/app/src/drive/panel.rs` | Existing left-side-ish panel; closest thing to a sidebar in the fork |

## Phase 3a — Brand strip (gate before any visible UI add)

**Goal:** Remove Warp/OpenWarp brand surfaces, replace with Meridian visual identity. This must happen BEFORE Phase 3b so we don't add Meridian-flavored UI alongside Warp wordmarks.

### Inputs needed from CTO
- Meridian wordmark (text or stylized — can borrow from `~/huang-design`?)
- Meridian app icon (SVG + .icns ideally)
- Primary brand color
- Font choice (or inherit Warp's)
- Bundle identifier (proposed: `dev.meridian.Meridian`)
- App display name (proposed: `Meridian`)

### Work items (Builder-dispatchable once CTO signs off on identity)
1. Audit `/app/assets/bundled/svg/` — identify the ~10–20 SVGs that are Warp/OpenWarp wordmarks or app icons (the rest are UI iconography we keep)
2. Replace identified SVGs with Meridian equivalents
3. Update `/app/assets/bundled/png/` channel badges (or remove if not needed)
4. Update `/app/Cargo.toml` `[package.metadata.bundle.bin.warp-oss]` block:
   - `name = "Meridian"` (was `"OpenWarp"`)
   - `identifier = "dev.meridian.Meridian"` (was `"dev.openwarp.OpenWarp"`)
   - `icon` paths point to new Meridian icons
   - `copyright` updated
5. Audit user-facing brand strings in `/app/src/` and `/crates/warp_core/src/` — replace "Warp"/"OpenWarp" where user-visible (NOT in code identifiers — those stay for compatibility with upstream merges)
6. Update `/app/src/themes/theme.rs` palette if we want Meridian brand color baked in
7. Rebuild + bundle: `cargo build --release --bin warp-oss` + `cargo bundle --release --bin warp-oss` → `Meridian.app` (the bundle name follows the metadata.bundle.name, so this will produce `Meridian.app` automatically)

### Out of scope for 3a
- Don't rename the binary (`warp-oss`) — that affects upstream merge surface
- Don't rename the channel (`Oss`) — same reason
- Don't rebrand internal crate names

### Estimated effort
1–2 Builder dispatches (~3–6 hours). The audit step is the biggest unknown; could surface more brand surfaces than the recon mapped.

## Phase 3b — Manager pane MVP

**Goal:** Add a `Manager` pane type that lets you open a Manager session inside the Meridian app, replacing the "open a Warp pane, run `claude`" workflow with a native Manager surface.

### MVP scope (smallest visible Meridian feature)
- New `Manager` variant in `IPaneType` enum at `/app/src/pane_group/pane/mod.rs`
- New file `/app/src/meridian/manager_pane.rs` (or wherever fits the codebase's organization) implementing `BackingView` + `PaneContent`
- Pane renders: scrollback area (Manager↔CTO chat transcript) + text input field
- Input submission calls `meridian_manager::dispatch_to(...)` (after Phase 2b) OR shells out to `script/meridian-dispatch.sh` (interim)
- Response appears in scrollback as Manager's reply
- Visual: matches existing AI chat pane style (`/app/src/ai/ai_document_view.rs` is the reference)

### Post-MVP (Phase 3b extras)
- Agent list pane / sidebar showing builder-1/2/3/laniakea status + last-dispatch time + click-to-open
- Relay traffic visualization in Builder/Lani panes — extend `AgentViewVisibility` to include relay blocks, or new block type
- Approval gate UI inline — when Manager flags a route as high-stakes, show modal or block-level prompt
- Manager-pane → Builder-pane click-to-jump

### Out of scope for 3b
- DAG / multi-step pipeline visualization (Phase 2 task DAG work; defer)
- Topology canvas (Phase 3c)
- Manager identity hot-reload (config-time only for now)

### Dependencies
- Phase 3a brand strip is the visual gate (don't add Meridian pane while wordmarks still say Warp)
- Phase 2b Rust port is nice-to-have but NOT a blocker — UI can shell out via `script/meridian-dispatch.sh` until 2b lands
- Recon findings on `IPaneType` + `BackingView` are the technical foundation

### Estimated effort
3–5 Builder dispatches (~1–2 evenings). The biggest unknowns are:
- Whether the AI chat pane pattern transfers cleanly to a Manager pane (probably yes; both are conversation surfaces)
- How `BackingView` handles async work (dispatched-claude calls take 10–30s; need a non-blocking UI pattern)

## Phase 3c — Canvas resurrection (own dedicated phase later)

Specs preserved from v1.5.0 at `~/meridian/specs/round3-4-ui-overhaul.md` + `~/meridian/specs/round2-panels-redesign.md` + `~/meridian/specs/meridian-v1-mockup.md`. Not Phase 3a/3b work.

## What Manager will do tonight vs later

**Tonight (this session):**
- Document this plan (done — this file)
- Update MERIDIAN.md to point at this plan
- Update manager-state.md open threads
- LOG to Lani about .app build + Phase 3 readiness (filing-worthy)

**Future sessions:**
- CTO decides Meridian visual identity
- Manager drafts Phase 3a Builder dispatch spec
- Builder executes 3a
- CTO ratifies the rebrand
- Manager drafts Phase 3b dispatch spec
- Builder executes 3b MVP
- Iterate on visible UX

## Notes
- The fork's UI uses `wgpu` (GPU-backed) + `winit` + custom Cocoa/NSView on macOS. Not egui, not slint, not GPUI. Internal abstraction in `crates/warpui/`.
- Pane-based architecture means every Meridian feature is a pane unless we explicitly add a new layout primitive (e.g., a left rail). For now, "Meridian feature = Manager pane + Agent list pane + Settings page" is the simplest mental model.
- Brand-strip is bigger than it sounds; 366 SVGs in the bundle. Most are UI iconography (e.g., terminal icons, command icons) that we KEEP. Only the wordmark / app-icon / channel-badge subset needs to be replaced.
- For the channel decision (extend `Oss` vs add `Meridian` variant): defer to Phase 3a. Adding a new channel means more divergence from upstream + more merge work; extending `Oss` keeps merges smaller. Lean toward extending unless CTO wants a hard fork.

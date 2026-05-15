# Spec — Phase 3a-1: Brand metadata + font port + black theme

> First visible Meridian/Hyperdrive change. Replaces upstream "OpenWarp" identity with **Hyperdrive** at the bundle/metadata + font/theme layer. SVG wordmark / app icon replacement is **deferred to a later round** (CTO hasn't provided Hyperdrive design assets yet; this round just renames + retones).

## Purpose
Land the smallest visible-aesthetic shift toward Hyperdrive. After this round, building the .app produces `Hyperdrive.app` (not `OpenWarp.app`), the bundle identifier shifts to `dev.hyperdrive.Hyperdrive`, the default font is Open Sauce Sans (v1.5.0 Meridian's primary font, ported), the terminal text rendering uses that proportional font (intentionally non-monospace per CTO), and the default theme is all-black.

## Scope (in this dispatch)

### 1. Port Open Sauce Sans fonts
Copy these 6 font files from v1.5.0 Meridian's bundle into `app/assets/bundled/fonts/`:

```
~/meridian/dist/fonts/OpenSauceSans-Regular.ttf     →   app/assets/bundled/fonts/
~/meridian/dist/fonts/OpenSauceSans-Medium.ttf      →   app/assets/bundled/fonts/
~/meridian/dist/fonts/OpenSauceSans-SemiBold.ttf    →   app/assets/bundled/fonts/
~/meridian/dist/fonts/OpenSauceSans-Bold.ttf        →   app/assets/bundled/fonts/
~/meridian/dist/fonts/OpenSauceOne-SemiBold.ttf     →   app/assets/bundled/fonts/
~/meridian/dist/fonts/OpenSauceOne-Bold.ttf         →   app/assets/bundled/fonts/
```

If `app/assets/bundled/fonts/` already has bundled fonts (likely — Warp ships JetBrains Mono or similar), DO NOT delete them. Add the new files alongside; the theme change in step 4 controls which is default.

### 2. Update bundle metadata for `warp-oss`
In `app/Cargo.toml`, update the existing `[package.metadata.bundle.bin.warp-oss]` block:

| Field | Was | Becomes |
|---|---|---|
| `name` | `"OpenWarp"` | `"Hyperdrive"` |
| `identifier` | `"dev.openwarp.OpenWarp"` | `"dev.hyperdrive.Hyperdrive"` |
| `copyright` | `"© 2025-2026, OpenWarp"` | `"© 2026, Hyperdrive"` |
| `short_description` | (existing OpenWarp blurb) | `"Hyperdrive — an AI-native developer terminal."` |
| `icon` | existing paths (channels/oss/icon/...) | **LEAVE AS-IS** — icon replacement is a future round |

Do NOT touch the metadata blocks for `[stable]`, `[preview]`, `[dev]`, `[warp]` — those are upstream-Warp-internal channels we don't build.

### 3. Audit font registration in Warp's theme/asset system
Warp's asset system (likely `asset_macro` + `asset_cache` crates per the workspace) discovers bundled assets at compile time. The font files you copy in step 1 should be picked up automatically by the existing macro/glob pattern — but verify by:
- `grep -r "JetBrains\|Mono\|Hack\|Inconsolata\|Geist\|Open Sauce" app/src crates/warp_terminal crates/warpui --include="*.rs"` to find where fonts are referenced
- Read `app/src/themes/theme.rs` and adjacent files in `app/src/themes/` to understand the font selection model

If the font registry is dynamic (reads `app/assets/bundled/fonts/` at runtime), step 1 alone wires it up. If it's static (hardcoded font names in Rust), you'll need to add `OpenSauceSans` / `OpenSauceOne` as registered families.

### 4. Default theme: all-black + Open Sauce Sans
- Find Warp's existing dark/black theme variant (likely in `app/src/themes/` — there should be `default.json`-style files or Rust constants)
- Either modify the default theme to be pure black background (`#000000`) OR set an existing dark variant as the default
- Set primary UI font to `Open Sauce Sans` (regular weight default; semibold/bold for emphasis)
- **Set terminal text font ALSO to Open Sauce Sans** — this is intentional per CTO; the terminal is non-monospace by design. Output alignment will break for tools like `top`/`htop`/`ls -la`/`git log --graph`. Accept the tradeoff.

If the theme system has separate "ui font" and "terminal font" settings, set both to Open Sauce Sans. Keep monospace fonts available as switchable options in settings (so users can switch back per-pane), but the default is the proportional font.

### 5. Verify build
After all changes:
```
cargo check -p warp_terminal -p warpui   # scoped first
cargo check --bin warp-oss               # then the binary
cargo build --release --bin warp-oss     # the real build
```

If all green, the bundle step (running `cargo bundle --release --bin warp-oss` from `app/` cwd) is Manager's responsibility — you don't need to bundle.

## Out of scope (deferred)

- **Wordmark / app-icon SVG replacement** — CTO will provide Hyperdrive design assets in a future round. Warp logo SVGs (`warp-logo-*.svg`, `warp.svg`, etc.) stay in place for now.
- **Default-pane rewrite** — Phase 3a-2, separate dispatch. Don't change default pane behavior in this round.
- **String audit of "Warp"/"OpenWarp" in user-facing UI text** — defer; the bundle metadata change is enough for the .app name to read "Hyperdrive."
- **Channel rename** (e.g., adding a `Hyperdrive` variant to `Channel` enum) — defer; we keep using `Oss` channel + `warp-oss` binary name to minimize upstream-merge friction.
- **Migrating Warp's internal "Warp" string literals to "Hyperdrive"** — too invasive; affects upstream merge surface.

## Tests / verification

No new unit tests required. Verification is:
- Scoped crates check cleanly (`cargo check -p ...`)
- `cargo build --release --bin warp-oss` completes
- Bundle metadata reflects Hyperdrive name + identifier
- Font files are present in `app/assets/bundled/fonts/`
- Theme defaults to all-black with Open Sauce Sans

## Files Builder will touch
- `app/Cargo.toml` (bundle metadata)
- `app/assets/bundled/fonts/` (6 new font files)
- Likely 1–3 files in `app/src/themes/` for default theme + font registration
- Possibly `app/src/themes/theme.rs` or equivalent
- Cargo.lock (auto-updates if any deps shift; commit alongside)

## Files Builder will NOT touch
- `app/assets/bundled/svg/` Warp wordmarks (deferred to icon-replacement round)
- `app/channels/oss/icon/` (deferred)
- `crates/warp_core/src/channel/mod.rs` (channel logic stays)
- Anything in `crates/warpui/` unless theme font selection lives there
- Any of the `meridian_*` crates

## Reporting
Return JSON as the final response:

```json
{
  "status": "ok|error",
  "commit_sha": "<git log -1 --format=%h>",
  "files_changed": ["app/Cargo.toml", "app/assets/bundled/fonts/...", "..."],
  "fonts_copied": 6,
  "theme_file_modified": "<path>",
  "cargo_check_warp_oss": "ok|fail",
  "notes": "<anything Manager should know — surprises, font-registration mechanism, deferred items>"
}
```

On error / blocker: `{"status":"error","step":"<which step>","reason":"<root cause>","files_already_modified":[...]}`.

## Notes
- Builder rule: NEVER push to origin. Manager handles the merge + rebuild + bundle.
- If theme system is more complex than this spec assumes (e.g., themes are loaded from external JSON files, not Rust constants), make minimum-viable changes that achieve the user-visible result (all-black default, Open Sauce Sans default) and document what you found.
- v1.5.0 Meridian app at `~/meridian/` is a frozen READ-ONLY reference for this round — you can `cat`/`grep` it but never edit it.
- This is the first visible aesthetic step. Subsequent rounds layer on (icon, wordmark, pane behavior, agent UI). Get this right and the rest gets easier.

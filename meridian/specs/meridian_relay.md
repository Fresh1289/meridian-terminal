# Spec — meridian_relay (Phase 1, leaf crate)

> First-real-logic slice. Builder dispatched: B1 (wt1).

## Purpose
Atomic relay processing + approval gates. The Manager↔Builder message bus.

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| Persistence layer | In-memory only this slice; JSONL append-to-disk in a follow-up | Keep slice small; v1.5.0 used SQLite, JSONL inherits Laniakea's contract |
| Async runtime | `tokio` with `mpsc` channels | Standard for Rust async, matches Warp upstream |
| Approval model | Relay carries `needs_approval: bool`; consumer holds via oneshot ack channel | Keeps consumer in control, doesn't require shared mutex |
| Message format | Plain struct, serde-serializable | JSONL persistence later needs serde anyway |

## Core Types

```rust
pub enum RelayType { Request, Report, Blocker, Fyi, Question }

pub struct Relay {
    pub id: Uuid,
    pub from: String,         // e.g. "Manager", "Builder 1"
    pub to: String,
    pub kind: RelayType,
    pub body: String,
    pub timestamp: DateTime<Utc>,
    pub needs_approval: bool,
}

pub struct RelayBus { /* sender + receiver halves */ }
```

## Public API
- `RelayBus::new() -> (RelaySender, RelayReceiver)`
- `RelaySender::send(relay: Relay) -> Result<(), RelayError>`
- `RelayReceiver::recv() -> Option<Relay>` (async)
- `RelayBus::approve(id: Uuid) -> Result<(), RelayError>` (resolves the oneshot for a needs_approval relay)

## Errors
`thiserror`-derived `RelayError` enum: `ChannelClosed`, `UnknownRelayId`, `AlreadyApproved`.

## Dependencies (Cargo.toml additions)
```toml
tokio = { version = "*", features = ["sync", "rt", "macros"] }
serde = { version = "*", features = ["derive"] }
chrono = { version = "*", features = ["serde"] }
uuid = { version = "*", features = ["v4", "serde"] }
thiserror = "*"
```
Use workspace versions if pinned in root Cargo.toml; otherwise let cargo pick latest compatible.

## First-Slice Scope

**IN:**
- `Relay`, `RelayType`, `RelayError` types (with serde derives)
- `RelayBus` with mpsc-backed send/recv
- Approval flow via oneshot ack
- 3 unit tests: send→recv roundtrip, approval-gate blocks until approved, unknown-id approve returns error

**OUT (defer):**
- JSONL persistence
- Multi-consumer fan-out
- Backpressure / bounded channels
- Authorization beyond the boolean flag

## Commit
Single atomic commit: `[Builder] Implement meridian_relay first-logic slice (Phase 1)`. Include `Cargo.toml`, `Cargo.lock`, `crates/meridian_relay/**`. Pre-commit gates mandatory.

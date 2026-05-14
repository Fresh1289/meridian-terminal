# Spec — meridian_laniakea (Phase 1, leaf crate)

> First-real-logic slice. Builder dispatched: B3 (wt3).
>
> **Important context:** This is the in-binary Rust crate that will eventually embed Laniakea's knowledge engine. It is SEPARATE from the standalone Laniakea CLI at `~/laniakea/`. The JSONL knowledge format is the contract between the two — both must read/write the same files.

## Purpose
Knowledge engine — read/write the JSONL knowledge stores Laniakea (the standalone CLI) maintains, exposing a query API.

## Design Decisions

| Question | Decision | Why |
|---|---|---|
| Read-side or write-side first | Both, but read is the larger surface | We need to query before we can advise |
| Storage format | JSONL append-only, one file per category | Contract with the standalone CLI; mirrors `~/laniakea/knowledge/*.jsonl` schema |
| In-memory vs streaming | Load full files into memory at startup; refresh on demand | Knowledge stores are tiny (KBs); simple is correct |
| Pattern detection | Deferred to round 4+ | First slice is just CRUD-shaped; pattern detection is a separate concern |

## Core Types

```rust
pub enum Category { Decision, Pattern, Failure, Preference, Insight }

pub struct KnowledgeEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub category: Category,
    pub summary: String,
    pub detail: String,
    pub domain: Vec<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub references: Vec<String>,
}

pub struct KnowledgeStore {
    root: PathBuf,
    entries: Vec<KnowledgeEntry>,
}

pub struct Query {
    pub category: Option<Category>,
    pub domain: Option<String>,
    pub tag: Option<String>,
    pub min_confidence: Option<f64>,
}
```

## Public API
- `KnowledgeStore::load(root: impl AsRef<Path>) -> Result<Self, KnowledgeError>`
- `fn query(&self, q: &Query) -> Vec<&KnowledgeEntry>`
- `async fn append(&mut self, entry: KnowledgeEntry) -> Result<(), KnowledgeError>` (writes to the right `<category>.jsonl` AND adds to in-memory `entries`)
- `async fn reload(&mut self) -> Result<(), KnowledgeError>`

## Errors
`thiserror`-derived `KnowledgeError`: `Io(#[from] std::io::Error)`, `Parse { line: usize, source: serde_json::Error }`, `MissingDir(PathBuf)`.

## Dependencies
```toml
serde = { version = "*", features = ["derive"] }
serde_json = "*"
chrono = { version = "*", features = ["serde"] }
uuid = { version = "*", features = ["v4", "serde"] }
tokio = { version = "*", features = ["fs", "rt", "macros"] }
thiserror = "*"
```

## First-Slice Scope

**IN:**
- All types above with serde derives matching the Laniakea CLI's JSONL schema (verify by reading `~/laniakea/knowledge/decisions.jsonl` for the canonical shape)
- `KnowledgeStore::load` reads all 5 category files (skip missing files gracefully)
- `query` filters by category/domain/tag/min_confidence
- `append` writes to disk (newline-delimited JSON) and updates in-memory state
- `reload` re-reads from disk
- 4 unit tests: load fixture → query roundtrips for each filter type, append-then-reload roundtrip
- Tests use `tempfile` for disk fixtures

**OUT (defer):**
- Pattern detection (round 4+)
- "Wisdom" advisory API (synthesis across entries)
- Multi-vault federation
- Indexing beyond linear scan (Vec is fine; <10K entries)

## Commit
Single atomic commit: `[Builder] Implement meridian_laniakea first-logic slice (Phase 1)`. Include `Cargo.toml`, `Cargo.lock`, `crates/meridian_laniakea/**`. Pre-commit gates mandatory.

## Cross-reference
Read `~/laniakea/CLAUDE.md` and one of `~/laniakea/knowledge/*.jsonl` for canonical schema before writing the serde derives. The schema MUST match — this crate and the standalone CLI both write to the same files in production.

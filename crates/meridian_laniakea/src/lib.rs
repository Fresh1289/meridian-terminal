use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Decision,
    Pattern,
    Failure,
    Preference,
    Insight,
}

impl Category {
    fn filename(self) -> &'static str {
        match self {
            Category::Decision => "decisions.jsonl",
            Category::Pattern => "patterns.jsonl",
            Category::Failure => "failures.jsonl",
            Category::Preference => "preferences.jsonl",
            Category::Insight => "insights.jsonl",
        }
    }

    const ALL: [Category; 5] = [
        Category::Decision,
        Category::Pattern,
        Category::Failure,
        Category::Preference,
        Category::Insight,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub timestamp: DateTime<FixedOffset>,
    pub category: Category,
    pub summary: String,
    pub detail: String,
    pub domain: Vec<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub references: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Query {
    pub category: Option<Category>,
    pub domain: Option<String>,
    pub tag: Option<String>,
    pub min_confidence: Option<f64>,
}

#[derive(Debug)]
pub struct KnowledgeStore {
    root: PathBuf,
    entries: Vec<KnowledgeEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error at line {line}")]
    Parse {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("knowledge directory not found: {0}")]
    MissingDir(PathBuf),
}

impl KnowledgeStore {
    pub async fn load(root: impl AsRef<Path>) -> Result<Self, KnowledgeError> {
        let root = root.as_ref().to_path_buf();
        if !tokio::fs::try_exists(&root).await? {
            return Err(KnowledgeError::MissingDir(root));
        }
        let entries = read_all_entries(&root).await?;
        Ok(Self { root, entries })
    }

    pub fn query(&self, q: &Query) -> Vec<&KnowledgeEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(cat) = q.category
                    && e.category != cat
                {
                    return false;
                }
                if let Some(domain) = &q.domain
                    && !e.domain.iter().any(|d| d == domain)
                {
                    return false;
                }
                if let Some(tag) = &q.tag
                    && !e.tags.iter().any(|t| t == tag)
                {
                    return false;
                }
                if let Some(min) = q.min_confidence
                    && e.confidence < min
                {
                    return false;
                }
                true
            })
            .collect()
    }

    pub async fn append(&mut self, entry: KnowledgeEntry) -> Result<(), KnowledgeError> {
        let path = self.root.join(entry.category.filename());
        let line = serde_json::to_string(&entry)
            .expect("KnowledgeEntry serialization is infallible (no maps with non-string keys)");
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        self.entries.push(entry);
        Ok(())
    }

    pub async fn reload(&mut self) -> Result<(), KnowledgeError> {
        self.entries = read_all_entries(&self.root).await?;
        Ok(())
    }
}

async fn read_all_entries(root: &Path) -> Result<Vec<KnowledgeEntry>, KnowledgeError> {
    let mut entries = Vec::new();
    for category in Category::ALL {
        let path = root.join(category.filename());
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        for (i, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: KnowledgeEntry =
                serde_json::from_str(line).map_err(|source| KnowledgeError::Parse {
                    line: i + 1,
                    source,
                })?;
            entries.push(entry);
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn entry(
        id: &str,
        cat: Category,
        domain: &[&str],
        tags: &[&str],
        confidence: f64,
    ) -> KnowledgeEntry {
        KnowledgeEntry {
            id: id.into(),
            timestamp: DateTime::parse_from_rfc3339("2026-05-14T10:00:00+08:00").unwrap(),
            category: cat,
            summary: format!("summary of {id}"),
            detail: format!("detail of {id}"),
            domain: domain.iter().map(|s| (*s).to_string()).collect(),
            tags: tags.iter().map(|s| (*s).to_string()).collect(),
            confidence,
            references: vec![],
        }
    }

    fn write_jsonl(root: &Path, filename: &str, entries: &[KnowledgeEntry]) {
        let body: String = entries
            .iter()
            .map(|e| {
                let mut s = serde_json::to_string(e).unwrap();
                s.push('\n');
                s
            })
            .collect();
        std::fs::write(root.join(filename), body).unwrap();
    }

    async fn fixture() -> (TempDir, KnowledgeStore) {
        let dir = TempDir::new().unwrap();
        write_jsonl(
            dir.path(),
            "decisions.jsonl",
            &[
                entry("d1", Category::Decision, &["meridian"], &["auth"], 0.9),
                entry("d2", Category::Decision, &["laniakea"], &["ui"], 0.6),
            ],
        );
        write_jsonl(
            dir.path(),
            "preferences.jsonl",
            &[entry(
                "p1",
                Category::Preference,
                &["meridian"],
                &["ui"],
                0.95,
            )],
        );
        let store = KnowledgeStore::load(dir.path()).await.unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn query_filters_by_category() {
        let (_dir, store) = fixture().await;
        let results = store.query(&Query {
            category: Some(Category::Decision),
            ..Default::default()
        });
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.category == Category::Decision));
    }

    #[tokio::test]
    async fn query_filters_by_domain_and_tag() {
        let (_dir, store) = fixture().await;
        let by_domain = store.query(&Query {
            domain: Some("meridian".into()),
            ..Default::default()
        });
        assert_eq!(by_domain.len(), 2);
        assert!(
            by_domain
                .iter()
                .all(|e| e.domain.contains(&"meridian".to_string()))
        );

        let by_tag = store.query(&Query {
            tag: Some("ui".into()),
            ..Default::default()
        });
        assert_eq!(by_tag.len(), 2);
        assert!(by_tag.iter().all(|e| e.tags.contains(&"ui".to_string())));
    }

    #[tokio::test]
    async fn query_filters_by_min_confidence() {
        let (_dir, store) = fixture().await;
        let results = store.query(&Query {
            min_confidence: Some(0.9),
            ..Default::default()
        });
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.confidence >= 0.9));
    }

    #[tokio::test]
    async fn append_then_reload_roundtrip() {
        let (dir, mut store) = fixture().await;
        let new_entry = entry("d3", Category::Decision, &["new"], &["test"], 0.7);
        store.append(new_entry).await.unwrap();
        assert_eq!(store.entries.len(), 4);

        let mut fresh = KnowledgeStore::load(dir.path()).await.unwrap();
        assert_eq!(fresh.entries.len(), 4);
        let found = fresh.entries.iter().find(|e| e.id == "d3").unwrap();
        assert_eq!(found.category, Category::Decision);
        assert!((found.confidence - 0.7).abs() < f64::EPSILON);

        fresh.reload().await.unwrap();
        assert_eq!(fresh.entries.len(), 4);
    }

    #[tokio::test]
    async fn load_missing_dir_is_error() {
        let err = KnowledgeStore::load("/nonexistent/path/that/should/not/exist")
            .await
            .unwrap_err();
        assert!(matches!(err, KnowledgeError::MissingDir(_)));
    }
}

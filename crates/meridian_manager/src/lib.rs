//! `meridian_manager` — Phase 1 wiring crate.
//!
//! Composes the four leaf crates (`meridian_relay`, `meridian_worktree`,
//! `meridian_agents`, `meridian_laniakea`) into a single `Manager`
//! orchestrator. No new domain logic — composition only.

use std::path::{Path, PathBuf};

use meridian_agents::{AgentError, AgentManager};
use meridian_laniakea::{KnowledgeError, KnowledgeStore};
use meridian_relay::{Relay, RelayBus, RelayError, RelayReceiver, RelaySender};
use meridian_worktree::{WorktreeError, WorktreeManager};

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

impl Manager {
    /// Construct the orchestrator. Both directories must already exist.
    pub async fn new(
        repo_root: impl Into<PathBuf>,
        knowledge_root: impl AsRef<Path>,
    ) -> Result<Self, ManagerError> {
        let (relay_sender, relay_receiver) = RelayBus::new();
        let worktrees = WorktreeManager::new(repo_root);
        let agents = AgentManager::new();
        let knowledge = KnowledgeStore::load(knowledge_root).await?;
        Ok(Self {
            relay_sender,
            relay_receiver,
            worktrees,
            agents,
            knowledge,
        })
    }

    /// Send relays to Builders.
    pub fn relay_sender(&self) -> &RelaySender {
        &self.relay_sender
    }

    /// Receive the next relay routed back to Manager. Returns `None` when
    /// the bus channel has closed.
    pub async fn recv_relay(&mut self) -> Option<Relay> {
        self.relay_receiver.recv().await
    }

    /// Read-only worktree-manager handle.
    pub fn worktrees(&self) -> &WorktreeManager {
        &self.worktrees
    }

    /// Mutable agent-manager handle (spawn/kill require `&mut`).
    pub fn agents_mut(&mut self) -> &mut AgentManager {
        &mut self.agents
    }

    /// Read-only knowledge-store handle for context queries.
    pub fn knowledge(&self) -> &KnowledgeStore {
        &self.knowledge
    }

    /// Gracefully kill every tracked agent. Returns the first error
    /// encountered; remaining agents are still kill-attempted.
    pub async fn shutdown(&mut self) -> Result<(), ManagerError> {
        let ids: Vec<_> = self.agents.list().iter().map(|a| a.id).collect();
        let mut first_err: Option<ManagerError> = None;
        for id in ids {
            if let Err(e) = self.agents.kill(id).await
                && first_err.is_none()
            {
                first_err = Some(e.into());
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use command::r#async::Command;
    use meridian_relay::{Relay, RelayType};
    use tempfile::TempDir;
    use uuid::Uuid;

    async fn init_git_repo(path: &Path) {
        let status = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(path)
            .status()
            .await
            .expect("git init runs");
        assert!(status.success(), "git init failed in {path:?}");
    }

    #[test]
    fn manager_identity_is_stable() {
        assert_eq!(MANAGER_IDENTITY, "Meridian Manager");
    }

    #[tokio::test]
    async fn manager_constructs_from_temp_environment() {
        let repo = TempDir::new().unwrap();
        let knowledge = TempDir::new().unwrap();
        init_git_repo(repo.path()).await;
        let manager = Manager::new(repo.path().to_path_buf(), knowledge.path())
            .await
            .expect("manager constructs");
        let results = manager
            .knowledge()
            .query(&meridian_laniakea::Query::default());
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn manager_routes_relay_roundtrip() {
        let repo = TempDir::new().unwrap();
        let knowledge = TempDir::new().unwrap();
        init_git_repo(repo.path()).await;
        let mut manager = Manager::new(repo.path().to_path_buf(), knowledge.path())
            .await
            .unwrap();

        let relay = Relay {
            id: Uuid::new_v4(),
            from: "Manager".into(),
            to: "Builder 1".into(),
            kind: RelayType::Request,
            body: "ping".into(),
            timestamp: chrono::Utc::now(),
            needs_approval: false,
        };
        let id = relay.id;
        manager.relay_sender().send(relay).expect("send ok");
        let received = manager.recv_relay().await.expect("recv yielded relay");
        assert_eq!(received.id, id);
    }

    #[tokio::test]
    async fn manager_shutdown_with_no_agents_is_clean() {
        let repo = TempDir::new().unwrap();
        let knowledge = TempDir::new().unwrap();
        init_git_repo(repo.path()).await;
        let mut manager = Manager::new(repo.path().to_path_buf(), knowledge.path())
            .await
            .unwrap();
        manager.shutdown().await.expect("shutdown ok");
    }

    #[tokio::test]
    async fn manager_knowledge_query_passthrough() {
        let repo = TempDir::new().unwrap();
        let knowledge = TempDir::new().unwrap();
        init_git_repo(repo.path()).await;

        let line = r#"{"id":"d1","timestamp":"2026-05-14T10:00:00+00:00","category":"decision","summary":"test decision","detail":"test detail","domain":["meridian"],"tags":["test"],"confidence":0.9,"references":[]}
"#;
        std::fs::write(knowledge.path().join("decisions.jsonl"), line).unwrap();

        let manager = Manager::new(repo.path().to_path_buf(), knowledge.path())
            .await
            .unwrap();
        let results = manager
            .knowledge()
            .query(&meridian_laniakea::Query::default());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "d1");
    }
}

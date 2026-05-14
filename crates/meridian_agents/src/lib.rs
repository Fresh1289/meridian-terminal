//! `meridian_agents` — Phase 1 first-logic slice.
//!
//! Spawns, tracks, and gracefully kills managed agent subprocesses. Each
//! agent runs inside a `meridian_worktree::Worktree` whose `CLAUDE.md` is
//! the source of truth for that agent's identity; role injection is purely
//! filesystem-based — no stdin prompt injection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use meridian_worktree::{Worktree, WorktreeError};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Spawning,
    Online,
    Idle,
    Killed,
}

pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub worktree: Worktree,
    pub state: AgentState,
    child: async_process::Child,
}

pub struct AgentManager {
    agents: HashMap<Uuid, Agent>,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("CLAUDE.md missing from worktree: {0}")]
    WorktreeMissingClaudeMd(PathBuf),
    #[error("spawn failed: {0}")]
    SpawnFailed(#[from] std::io::Error),
    #[error("unknown agent id: {0}")]
    UnknownAgentId(Uuid),
    #[error("kill timed out before graceful exit")]
    KillTimeout,
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
}

const KILL_GRACE: Duration = Duration::from_secs(5);

impl AgentManager {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub async fn spawn(
        &mut self,
        name: &str,
        worktree: Worktree,
        command: &str,
    ) -> Result<Uuid, AgentError> {
        let claude_md = worktree.path.join("CLAUDE.md");
        if !claude_md.exists() {
            return Err(AgentError::WorktreeMissingClaudeMd(claude_md));
        }

        let mut cmd = Command::new(command);
        cmd.current_dir(&worktree.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = cmd.spawn()?;

        let id = Uuid::new_v4();
        let agent = Agent {
            id,
            name: name.to_string(),
            worktree,
            state: AgentState::Online,
            child,
        };
        self.agents.insert(id, agent);
        Ok(id)
    }

    pub async fn kill(&mut self, id: Uuid) -> Result<(), AgentError> {
        let mut agent = self
            .agents
            .remove(&id)
            .ok_or(AgentError::UnknownAgentId(id))?;
        agent.state = AgentState::Killed;

        request_graceful_exit(&agent.child);

        match tokio::time::timeout(KILL_GRACE, agent.child.status()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(AgentError::SpawnFailed(e)),
            Err(_) => {
                let _ = agent.child.kill();
                Ok(())
            }
        }
    }

    pub fn list(&self) -> Vec<&Agent> {
        self.agents.values().collect()
    }

    pub fn get(&self, id: Uuid) -> Option<&Agent> {
        self.agents.get(&id)
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
fn request_graceful_exit(child: &async_process::Child) {
    let pid = child.id() as i32;
    // SAFETY: `libc::kill` with a known child pid is async-signal-safe and
    // returns an error code rather than panicking for invalid pids.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}

#[cfg(not(unix))]
fn request_graceful_exit(_child: &async_process::Child) {
    // No standard graceful-exit primitive on non-unix targets in this slice.
    // The 5-second timeout will fall through to forced kill.
}

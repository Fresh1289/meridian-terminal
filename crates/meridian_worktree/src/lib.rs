//! `meridian_worktree` — Phase 1 first-logic slice.
//!
//! Wraps `git worktree` operations so the orchestrator can isolate each
//! spawned agent in its own working directory. Branch creation is the
//! caller's responsibility; this crate only manages worktree lifecycle.

use std::path::{Path, PathBuf};

use command::r#async::Command;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub branch: String,
    pub path: PathBuf,
}

pub struct WorktreeManager {
    repo_root: PathBuf,
}

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("git command failed: {stderr}")]
    GitFailed { stderr: String },
    #[error("path already exists: {0}")]
    PathExists(PathBuf),
    #[error("branch missing: {0}")]
    BranchMissing(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl WorktreeManager {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    pub async fn add(&self, branch: &str) -> Result<Worktree, WorktreeError> {
        let path = self.worktree_path_for(branch);
        if path.exists() {
            return Err(WorktreeError::PathExists(path));
        }
        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .arg("worktree")
            .arg("add")
            .arg(&path)
            .arg(branch)
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            if stderr.contains("invalid reference") || stderr.contains("not a valid object name") {
                return Err(WorktreeError::BranchMissing(branch.to_string()));
            }
            return Err(WorktreeError::GitFailed { stderr });
        }
        Ok(Worktree {
            branch: branch.to_string(),
            path,
        })
    }

    pub async fn remove(
        &self,
        worktree: &Worktree,
        delete_branch: bool,
    ) -> Result<(), WorktreeError> {
        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&worktree.path)
            .output()
            .await?;
        if !output.status.success() {
            return Err(WorktreeError::GitFailed {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if delete_branch {
            let output = Command::new("git")
                .current_dir(&self.repo_root)
                .arg("branch")
                .arg("-D")
                .arg(&worktree.branch)
                .output()
                .await?;
            if !output.status.success() {
                return Err(WorktreeError::GitFailed {
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                });
            }
        }
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<Worktree>, WorktreeError> {
        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .arg("worktree")
            .arg("list")
            .arg("--porcelain")
            .output()
            .await?;
        if !output.status.success() {
            return Err(WorktreeError::GitFailed {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut current_path: Option<PathBuf> = None;
        let mut current_branch: Option<String> = None;
        for line in stdout.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                current_path = Some(PathBuf::from(p));
            } else if let Some(b) = line.strip_prefix("branch ") {
                current_branch = Some(b.trim_start_matches("refs/heads/").to_string());
            } else if line.is_empty()
                && let (Some(path), Some(branch)) = (current_path.take(), current_branch.take())
            {
                worktrees.push(Worktree { branch, path });
            }
        }
        if let (Some(path), Some(branch)) = (current_path, current_branch) {
            worktrees.push(Worktree { branch, path });
        }
        Ok(worktrees)
    }

    fn worktree_path_for(&self, branch: &str) -> PathBuf {
        let parent = self.repo_root.parent().unwrap_or(Path::new("."));
        let repo_name = self
            .repo_root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".to_string());
        parent.join(format!("{repo_name}-{branch}"))
    }
}

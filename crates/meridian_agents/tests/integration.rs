use std::path::Path;

use meridian_agents::{AgentError, AgentManager, AgentState};
use meridian_worktree::{Worktree, WorktreeManager};
use tempfile::TempDir;

async fn git(repo_root: &Path, args: &[&str]) {
    let output = command::r#async::Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .await
        .expect("git binary available");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn setup_worktree(branch: &str) -> (TempDir, Worktree) {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path().join("repo");
    std::fs::create_dir(&repo_root).expect("mkdir repo_root");
    git(&repo_root, &["init", "--initial-branch=main"]).await;
    git(&repo_root, &["config", "user.email", "agents@test.local"]).await;
    git(&repo_root, &["config", "user.name", "agents-test"]).await;
    git(&repo_root, &["commit", "--allow-empty", "-m", "init"]).await;
    git(&repo_root, &["branch", branch]).await;

    let manager = WorktreeManager::new(&repo_root);
    let worktree = manager.add(branch).await.expect("add worktree");
    (tmp, worktree)
}

#[tokio::test]
async fn spawn_kill_lifecycle() {
    let (_tmp, worktree) = setup_worktree("agent-lifecycle").await;
    std::fs::write(worktree.path.join("CLAUDE.md"), "test identity\n").expect("write CLAUDE.md");

    let mut manager = AgentManager::new();
    let id = manager
        .spawn("Builder 1", worktree, "cat")
        .await
        .expect("spawn ok");

    let agent = manager.get(id).expect("agent registered");
    assert_eq!(agent.state, AgentState::Online);
    assert_eq!(agent.name, "Builder 1");
    assert_eq!(manager.list().len(), 1);

    manager.kill(id).await.expect("kill ok");
    assert!(
        manager.get(id).is_none(),
        "agent should be removed after kill"
    );
    assert!(manager.list().is_empty());
}

#[tokio::test]
async fn spawn_missing_claude_md_errors() {
    let (_tmp, worktree) = setup_worktree("agent-no-claudemd").await;
    // Intentionally do NOT write CLAUDE.md.

    let mut manager = AgentManager::new();
    let result = manager.spawn("Builder 1", worktree, "cat").await;

    match result {
        Err(AgentError::WorktreeMissingClaudeMd(path)) => {
            assert!(
                path.ends_with("CLAUDE.md"),
                "error should reference CLAUDE.md path"
            );
        }
        other => panic!("expected WorktreeMissingClaudeMd, got {other:?}"),
    }
}

#[tokio::test]
async fn double_kill_returns_unknown_id() {
    let (_tmp, worktree) = setup_worktree("agent-doublekill").await;
    std::fs::write(worktree.path.join("CLAUDE.md"), "test\n").expect("write CLAUDE.md");

    let mut manager = AgentManager::new();
    let id = manager
        .spawn("Builder 1", worktree, "cat")
        .await
        .expect("spawn ok");
    manager.kill(id).await.expect("first kill ok");

    let result = manager.kill(id).await;
    assert!(
        matches!(result, Err(AgentError::UnknownAgentId(missing)) if missing == id),
        "expected UnknownAgentId for id {id}, got {result:?}",
    );
}

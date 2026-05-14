use std::path::{Path, PathBuf};

use command::blocking::Command;
use futures::executor::block_on;
use meridian_worktree::{WorktreeError, WorktreeManager};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args(args)
        .status()
        .expect("git invocation");
    assert!(status.success(), "git {args:?} failed");
}

fn init_repo() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let repo = dir
        .path()
        .canonicalize()
        .expect("canonicalize tempdir")
        .join("repo");
    std::fs::create_dir(&repo).expect("mkdir repo");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["commit", "--allow-empty", "-q", "-m", "init"]);
    git(&repo, &["branch", "feature"]);
    (dir, repo)
}

#[test]
fn add_then_list_contains_new_worktree() {
    let (_tmp, repo) = init_repo();
    let mgr = WorktreeManager::new(&repo);
    let wt = block_on(mgr.add("feature")).expect("add");
    let listed = block_on(mgr.list()).expect("list");
    assert!(listed.iter().any(|w| w.branch == "feature"));
    assert_eq!(wt.branch, "feature");
    block_on(mgr.remove(&wt, false)).expect("remove");
}

#[test]
fn add_then_remove_removes_worktree() {
    let (_tmp, repo) = init_repo();
    let mgr = WorktreeManager::new(&repo);
    let wt = block_on(mgr.add("feature")).expect("add");
    block_on(mgr.remove(&wt, true)).expect("remove");
    let listed = block_on(mgr.list()).expect("list");
    assert!(listed.iter().all(|w| w.branch != "feature"));
}

#[test]
fn add_twice_same_branch_returns_path_exists() {
    let (_tmp, repo) = init_repo();
    let mgr = WorktreeManager::new(&repo);
    let wt = block_on(mgr.add("feature")).expect("add");
    let err = block_on(mgr.add("feature")).expect_err("expected PathExists");
    assert!(matches!(err, WorktreeError::PathExists(_)));
    let _ = block_on(mgr.remove(&wt, false));
}

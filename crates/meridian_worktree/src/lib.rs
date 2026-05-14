pub fn worktree_identity() -> &'static str {
    "meridian_worktree"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable() {
        assert_eq!(worktree_identity(), "meridian_worktree");
    }
}

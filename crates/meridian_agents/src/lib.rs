//! `meridian_agents` — Phase 1 scaffold.
//!
//! Future home of agent spawning, lifecycle, role injection, and graceful
//! kill logic. This crate intentionally ships only an identity stub until
//! the orchestration spec lands.

pub fn agents_identity() -> &'static str {
    "meridian_agents"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable() {
        assert_eq!(agents_identity(), "meridian_agents");
    }
}

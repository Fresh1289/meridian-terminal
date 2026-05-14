//! `meridian_relay` — Phase 1 scaffold.
//!
//! Future home of the Manager↔Builder message bus: atomic relay
//! processing and human-in-the-loop approval gates. This crate
//! intentionally ships only an identity stub until the relay spec lands.

pub fn relay_identity() -> &'static str {
    "meridian_relay"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable() {
        assert_eq!(relay_identity(), "meridian_relay");
    }
}

pub fn laniakea_identity() -> &'static str {
    "meridian_laniakea"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable() {
        assert_eq!(laniakea_identity(), "meridian_laniakea");
    }
}

pub fn manager_identity() -> &'static str {
    "Meridian Manager"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_identity_is_stable() {
        assert_eq!(manager_identity(), "Meridian Manager");
    }
}

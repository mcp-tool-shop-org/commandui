pub mod errors;
pub mod events;
#[cfg(test)]
mod parity;
pub mod pty;
pub mod services;
pub mod session;

/// Crate version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION should be semver: {VERSION}");
        for part in &parts {
            part.parse::<u32>().unwrap_or_else(|_| panic!("Not a number: {part}"));
        }
    }

    #[test]
    fn version_at_least_1_0_0() {
        let major: u32 = VERSION.split('.').next().unwrap().parse().unwrap();
        assert!(major >= 1, "Expected major >= 1, got {major}");
    }
}

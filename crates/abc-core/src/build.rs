//! Which build this is, to tell builds apart when abc and abcd meet.

/// The version and a hash of the sources it was built from, such as
/// `0.1.0+3f9a2c1e`. Every rebuild from changed sources changes it.
pub const BUILD_ID: &str = env!("ABC_BUILD_ID");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_id_is_the_version_and_a_hash() {
        let (version, hash) = BUILD_ID.split_once('+').expect("a +");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
        assert_eq!(hash.len(), 8);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

//! `no-claude-trailer` is dependency free, and this test keeps it that way.
//!
//! The promise is on the front of the README, so it is enforced rather than
//! remembered: add a `[dependencies]` entry and this test fails.

#[test]
fn the_crate_has_no_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    for line in manifest.lines() {
        let section = line.trim();
        assert!(
            section != "[dependencies]" && section != "[build-dependencies]",
            "no-claude-trailer is dependency free; {section} must stay empty"
        );
    }
}

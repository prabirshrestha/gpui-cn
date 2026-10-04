//! The smallest end-to-end check: write text, read it back.

mod common;

use ghostty_vt::build_info;

#[test]
fn writes_hi_and_reads_it_back() {
    let mut terminal = common::terminal(10, 2);
    terminal.vt_write(b"hi");
    assert_eq!(common::rows(&terminal), vec!["hi", ""]);
    assert_eq!(terminal.cursor_x().unwrap(), 2);
    assert_eq!(terminal.cursor_y().unwrap(), 0);
}

#[test]
fn links_the_pinned_ghostty() {
    let lock = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ghostty-vt-sys/GHOSTTY.lock"),
    )
    .unwrap();
    let field = |name: &str| {
        lock.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(" = "))
            .unwrap()
            .to_owned()
    };
    assert_eq!(build_info::pinned_commit(), field("commit"));
    assert_eq!(build_info::pinned_version(), field("version"));
    // The library carries libghostty-vt's own version, not Ghostty's.
    let version = build_info::version_string().unwrap();
    assert!(
        version.starts_with(|c: char| c.is_ascii_digit()),
        "{version}"
    );
    assert_eq!(
        version.split(['-', '+']).next().unwrap(),
        format!(
            "{}.{}.{}",
            build_info::major_version().unwrap(),
            build_info::minor_version().unwrap(),
            build_info::patch_version().unwrap()
        )
    );
    assert!(build_info::supports_simd().unwrap());
    assert!(build_info::supports_kitty_graphics().unwrap());
    assert_eq!(
        build_info::optimize_mode().unwrap(),
        build_info::OptimizeMode::ReleaseFast
    );
}

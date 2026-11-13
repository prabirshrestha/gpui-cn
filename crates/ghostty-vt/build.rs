//! Check that this crate identifies the Ghostty source used by ghostty-vt-sys.

use std::env;

fn main() {
    let expected =
        env::var("DEP_GHOSTTY_VT_BUILD_METADATA").expect("DEP_GHOSTTY_VT_BUILD_METADATA");
    let package_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let metadata = package_version.split_once('+').map(|(_, value)| value);
    assert_eq!(
        metadata,
        Some(expected.as_str()),
        "ghostty-vt version must end in +{expected}, the build metadata of ghostty-vt-sys"
    );
}

/// The human readable name of the client
pub const NAME_CLIENT: &str = "Brontes";

/// The latest version from Cargo.toml.
pub const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The full SHA of the latest commit.
pub const VERGEN_GIT_SHA_LONG: &str = env!("VERGEN_GIT_SHA");

/// The 8 character short SHA of the latest commit.
pub const VERGEN_GIT_SHA: &str = const_format::str_index!(VERGEN_GIT_SHA_LONG, ..8);

/// The build timestamp.
pub const VERGEN_BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");

/// The target triple.
pub const VERGEN_CARGO_TARGET_TRIPLE: &str = env!("VERGEN_CARGO_TARGET_TRIPLE");

/// The build features.
pub const VERGEN_CARGO_FEATURES: &str = env!("VERGEN_CARGO_FEATURES");

/// The short version information for brontes.
///
/// - The latest version from Cargo.toml
/// - The short SHA of the latest commit.
///
/// # Example
///
/// ```text
/// 0.1.0 (defa64b2)
/// ```
pub const SHORT_VERSION: &str = const_format::concatcp!(
    env!("CARGO_PKG_VERSION"),
    env!("BRONTES_VERSION_SUFFIX"),
    " (",
    VERGEN_GIT_SHA,
    ")"
);

/// The long version information for brontes.
///
/// - The latest version from Cargo.toml
/// - The long SHA of the latest commit.
/// - The build datetime
/// - The build features
/// - The build profile
///
/// # Example:
///
/// ```text
/// Version: 0.1.0
/// Commit SHA: defa64b2
/// Build Timestamp: 2023-05-19T01:47:19.815651705Z
/// Build Features: jemalloc
/// Build Profile: maxperf
/// ```
pub const LONG_VERSION: &str = const_format::concatcp!(
    "Version: ",
    env!("CARGO_PKG_VERSION"),
    env!("BRONTES_VERSION_SUFFIX"),
    "\n",
    "Commit SHA: ",
    VERGEN_GIT_SHA_LONG,
    "\n",
    "Build Timestamp: ",
    env!("VERGEN_BUILD_TIMESTAMP"),
    "\n",
    "Build Features: ",
    env!("VERGEN_CARGO_FEATURES"),
    "\n",
    "Build Profile: ",
    BUILD_PROFILE_NAME
);

/// The build profile name.
pub const BUILD_PROFILE_NAME: &str = {
    // Derived from https://stackoverflow.com/questions/73595435/how-to-get-profile-from-cargo-toml-in-build-rs-or-at-runtime
    // We split on the path separator of the *host* machine, which may be different
    // from `std::path::MAIN_SEPARATOR_STR`.
    const OUT_DIR: &str = env!("OUT_DIR");
    let unix_parts = const_format::str_split!(OUT_DIR, '/');
    if unix_parts.len() >= 4 {
        unix_parts[unix_parts.len() - 4]
    } else {
        let win_parts = const_format::str_split!(OUT_DIR, '\\');
        win_parts[win_parts.len() - 4]
    }
};

//! Which library the build script links, chosen from the environment, and
//! where it reads the release files from.
//!
//! Precedence, first match wins:
//!
//! 1. `GHOSTTY_SOURCE_DIR`: a source build of that tree.
//! 2. `GHOSTTY_VT_FROM_SOURCE=1`: a source build of the pinned release.
//! 3. `GHOSTTY_VT_LIB_DIR`: a static library already on disk.
//! 4. `GHOSTTY_VT_PREBUILT_URL` with `GHOSTTY_VT_PREBUILT_SHA256`: a custom
//!    archive in the layout of the target's release file.
//! 5. The pinned release: its prebuilt library where the target has one,
//!    else a source build. Release files come from `GHOSTTY_RELEASE_DIR`,
//!    else from `GHOSTTY_RELEASE_URL`, else from the URL in `GHOSTTY.lock`.
//!
//! An override that a higher one hides is reported as a warning.

/// The overrides the build script reads. An empty variable counts as unset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Env {
    pub source_dir: Option<String>,
    pub from_source: bool,
    pub lib_dir: Option<String>,
    pub lib_sha256: Option<String>,
    pub prebuilt_url: Option<String>,
    pub prebuilt_sha256: Option<String>,
    pub release_dir: Option<String>,
    pub release_url: Option<String>,
}

impl Env {
    /// Read the overrides with `var`, which returns a variable's value.
    pub fn read(var: impl Fn(&str) -> Option<String>) -> Self {
        let value = |name: &str| var(name).filter(|value| !value.is_empty());
        Self {
            source_dir: value("GHOSTTY_SOURCE_DIR"),
            from_source: value("GHOSTTY_VT_FROM_SOURCE").is_some_and(|value| value != "0"),
            lib_dir: value("GHOSTTY_VT_LIB_DIR"),
            lib_sha256: value("GHOSTTY_VT_LIB_SHA256"),
            prebuilt_url: value("GHOSTTY_VT_PREBUILT_URL"),
            prebuilt_sha256: value("GHOSTTY_VT_PREBUILT_SHA256"),
            release_dir: value("GHOSTTY_RELEASE_DIR"),
            release_url: value("GHOSTTY_RELEASE_URL"),
        }
    }
}

/// The library to link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Library {
    /// Build this source tree with Zig.
    SourceDir(String),
    /// Build the pinned release from source with Zig.
    FromSource,
    /// Link the static library in this directory, checked against the
    /// sha256 when one is given.
    LibDir { dir: String, sha256: Option<String> },
    /// Download this archive and check it against the sha256.
    PrebuiltUrl { url: String, sha256: String },
    /// The pinned release, prebuilt where the target has a library.
    Release,
}

/// The library `env` selects, and a warning for each override it ignores.
pub fn choose(env: &Env) -> Result<(Library, Vec<String>), String> {
    let lib_sha256 = env
        .lib_sha256
        .as_deref()
        .map(|sha| sha256("GHOSTTY_VT_LIB_SHA256", sha))
        .transpose()?;
    if lib_sha256.is_some() && env.lib_dir.is_none() {
        return Err("GHOSTTY_VT_LIB_SHA256 is set without GHOSTTY_VT_LIB_DIR".into());
    }
    let prebuilt = match (&env.prebuilt_url, &env.prebuilt_sha256) {
        (Some(url), Some(sha)) => Some((url.clone(), sha256("GHOSTTY_VT_PREBUILT_SHA256", sha)?)),
        (Some(_), None) => {
            return Err(
                "GHOSTTY_VT_PREBUILT_URL is set without GHOSTTY_VT_PREBUILT_SHA256, the sha256 \
                 of the archive"
                    .into(),
            );
        }
        (None, Some(_)) => {
            return Err("GHOSTTY_VT_PREBUILT_SHA256 is set without GHOSTTY_VT_PREBUILT_URL".into());
        }
        (None, None) => None,
    };

    // The candidates in precedence order, each with its variable's name.
    let mut candidates: Vec<(&str, Library)> = Vec::new();
    if let Some(dir) = &env.source_dir {
        candidates.push(("GHOSTTY_SOURCE_DIR", Library::SourceDir(dir.clone())));
    }
    if env.from_source {
        candidates.push(("GHOSTTY_VT_FROM_SOURCE", Library::FromSource));
    }
    if let Some(dir) = &env.lib_dir {
        candidates.push((
            "GHOSTTY_VT_LIB_DIR",
            Library::LibDir {
                dir: dir.clone(),
                sha256: lib_sha256,
            },
        ));
    }
    if let Some((url, sha256)) = prebuilt {
        candidates.push((
            "GHOSTTY_VT_PREBUILT_URL",
            Library::PrebuiltUrl { url, sha256 },
        ));
    }

    let mut candidates = candidates.into_iter();
    let Some((chosen, library)) = candidates.next() else {
        return Ok((Library::Release, release_warnings(env)));
    };
    let mut warnings: Vec<String> = candidates
        .map(|(name, _)| format!("{name} is ignored: {chosen} takes precedence"))
        .collect();
    // Release files are still read by a source build of the release.
    if library == Library::FromSource {
        warnings.extend(release_warnings(env));
    } else {
        for name in [
            ("GHOSTTY_RELEASE_DIR", env.release_dir.is_some()),
            ("GHOSTTY_RELEASE_URL", env.release_url.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, set)| set.then_some(name))
        {
            warnings.push(format!("{name} is ignored: {chosen} takes precedence"));
        }
    }
    Ok((library, warnings))
}

fn release_warnings(env: &Env) -> Vec<String> {
    if env.release_dir.is_some() && env.release_url.is_some() {
        vec!["GHOSTTY_RELEASE_URL is ignored: GHOSTTY_RELEASE_DIR takes precedence".into()]
    } else {
        Vec::new()
    }
}

/// A lower-case sha256, or an error naming the variable.
fn sha256(name: &str, value: &str) -> Result<String, String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(format!("{name} is not a sha256 (64 hex digits): {value}"))
    }
}

/// Where a release file is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A path on disk.
    Path(String),
    /// A URL to download.
    Url(String),
}

/// Where the release file `name` comes from: `GHOSTTY_RELEASE_DIR`, else
/// the `GHOSTTY_RELEASE_URL` mirror, else `default_base`, the release URL
/// in `GHOSTTY.lock`.
pub fn release_origin(env: &Env, default_base: &str, name: &str) -> Origin {
    if let Some(dir) = &env.release_dir {
        return Origin::Path(format!("{}/{name}", dir.trim_end_matches(['/', '\\'])));
    }
    let base = env.release_url.as_deref().unwrap_or(default_base);
    Origin::Url(format!("{}/{name}", base.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::{Env, Library, Origin, choose, release_origin};

    const SHA: &str = "8e2eb9e1822e85d786b456c8beb86f49d09c318c91f0c2913f91e7ffa12cda29";

    fn env(vars: &[(&str, &str)]) -> Env {
        Env::read(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn no_override_links_the_pinned_release() {
        assert_eq!(choose(&env(&[])), Ok((Library::Release, vec![])));
        assert_eq!(
            choose(&env(&[
                ("GHOSTTY_VT_LIB_DIR", ""),
                ("GHOSTTY_VT_FROM_SOURCE", "0")
            ])),
            Ok((Library::Release, vec![]))
        );
    }

    #[test]
    fn overrides_follow_their_precedence() {
        let all = [
            ("GHOSTTY_SOURCE_DIR", "/src"),
            ("GHOSTTY_VT_FROM_SOURCE", "1"),
            ("GHOSTTY_VT_LIB_DIR", "/lib"),
            ("GHOSTTY_VT_PREBUILT_URL", "https://m/a.zip"),
            ("GHOSTTY_VT_PREBUILT_SHA256", SHA),
        ];
        let chosen = |count: usize| choose(&env(&all[all.len() - count..])).unwrap().0;
        assert_eq!(
            chosen(2),
            Library::PrebuiltUrl {
                url: "https://m/a.zip".into(),
                sha256: SHA.into(),
            }
        );
        assert_eq!(
            chosen(3),
            Library::LibDir {
                dir: "/lib".into(),
                sha256: None,
            }
        );
        assert_eq!(chosen(4), Library::FromSource);
        assert_eq!(chosen(5), Library::SourceDir("/src".into()));
        let (_, warnings) = choose(&env(&all)).unwrap();
        assert_eq!(
            warnings,
            [
                "GHOSTTY_VT_FROM_SOURCE is ignored: GHOSTTY_SOURCE_DIR takes precedence",
                "GHOSTTY_VT_LIB_DIR is ignored: GHOSTTY_SOURCE_DIR takes precedence",
                "GHOSTTY_VT_PREBUILT_URL is ignored: GHOSTTY_SOURCE_DIR takes precedence",
            ]
        );
    }

    #[test]
    fn release_overrides_are_reported_when_unused() {
        let (library, warnings) = choose(&env(&[
            ("GHOSTTY_VT_LIB_DIR", "/lib"),
            ("GHOSTTY_RELEASE_DIR", "/rel"),
        ]))
        .unwrap();
        assert!(matches!(library, Library::LibDir { .. }));
        assert_eq!(
            warnings,
            ["GHOSTTY_RELEASE_DIR is ignored: GHOSTTY_VT_LIB_DIR takes precedence"]
        );
        // A source build of the release still reads the release files.
        let (_, warnings) = choose(&env(&[
            ("GHOSTTY_VT_FROM_SOURCE", "1"),
            ("GHOSTTY_RELEASE_URL", "https://m"),
        ]))
        .unwrap();
        assert!(warnings.is_empty());
        let (_, warnings) = choose(&env(&[
            ("GHOSTTY_RELEASE_DIR", "/rel"),
            ("GHOSTTY_RELEASE_URL", "https://m"),
        ]))
        .unwrap();
        assert_eq!(
            warnings,
            ["GHOSTTY_RELEASE_URL is ignored: GHOSTTY_RELEASE_DIR takes precedence"]
        );
    }

    #[test]
    fn a_prebuilt_url_needs_its_sha256() {
        assert!(choose(&env(&[("GHOSTTY_VT_PREBUILT_URL", "https://m/a.zip")])).is_err());
        assert!(choose(&env(&[("GHOSTTY_VT_PREBUILT_SHA256", SHA)])).is_err());
        assert!(
            choose(&env(&[
                ("GHOSTTY_VT_PREBUILT_URL", "https://m/a.zip"),
                ("GHOSTTY_VT_PREBUILT_SHA256", "abc"),
            ]))
            .is_err()
        );
        let upper = SHA.to_ascii_uppercase();
        let (library, _) = choose(&env(&[
            ("GHOSTTY_VT_PREBUILT_URL", "https://m/a.zip"),
            ("GHOSTTY_VT_PREBUILT_SHA256", &upper),
        ]))
        .unwrap();
        assert_eq!(
            library,
            Library::PrebuiltUrl {
                url: "https://m/a.zip".into(),
                sha256: SHA.into(),
            }
        );
    }

    #[test]
    fn a_lib_sha256_needs_the_lib_dir() {
        assert!(choose(&env(&[("GHOSTTY_VT_LIB_SHA256", SHA)])).is_err());
        assert!(
            choose(&env(&[
                ("GHOSTTY_VT_LIB_DIR", "/lib"),
                ("GHOSTTY_VT_LIB_SHA256", "xyz"),
            ]))
            .is_err()
        );
        let (library, _) = choose(&env(&[
            ("GHOSTTY_VT_LIB_DIR", "/lib"),
            ("GHOSTTY_VT_LIB_SHA256", SHA),
        ]))
        .unwrap();
        assert_eq!(
            library,
            Library::LibDir {
                dir: "/lib".into(),
                sha256: Some(SHA.into()),
            }
        );
    }

    #[test]
    fn release_files_come_from_the_dir_then_the_mirror_then_the_lock() {
        let base = "https://release.files.ghostty.org/libghostty-vt/1.0.1";
        assert_eq!(
            release_origin(&env(&[]), base, "a.zip"),
            Origin::Url(format!("{base}/a.zip"))
        );
        assert_eq!(
            release_origin(
                &env(&[("GHOSTTY_RELEASE_URL", "https://m/vt/")]),
                base,
                "a.zip"
            ),
            Origin::Url("https://m/vt/a.zip".into())
        );
        assert_eq!(
            release_origin(
                &env(&[
                    ("GHOSTTY_RELEASE_DIR", "/rel/"),
                    ("GHOSTTY_RELEASE_URL", "https://m"),
                ]),
                base,
                "a.zip"
            ),
            Origin::Path("/rel/a.zip".into())
        );
    }
}

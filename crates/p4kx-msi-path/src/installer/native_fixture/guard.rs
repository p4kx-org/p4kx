// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

/// The current disposable runner and a fixed task directory beneath its temporary root.
#[derive(Debug)]
pub struct FixtureGuard {
    root: PathBuf,
    run: String,
    attempt: String,
    revision: String,
}

impl FixtureGuard {
    /// Validates the opt-in runner boundary without accessing the Registry.
    ///
    /// # Errors
    /// Returns invalid runner identity, source revision or redirected directory errors.
    pub fn open(root: &Path) -> io::Result<Self> {
        let (run, attempt, revision) = environment()?;
        let runner_temp = std::env::var_os("RUNNER_TEMP").ok_or_else(refused)?;
        let runner_temp = checked_directory(Path::new(&runner_temp))?;
        let root = checked_directory(root)?;
        if root == runner_temp || !root.starts_with(&runner_temp) {
            return Err(refused());
        }
        Ok(Self {
            root,
            run,
            attempt,
            revision,
        })
    }

    /// Returns the fixed task root validated for this invocation.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the hosted run identifier.
    #[must_use]
    pub fn run(&self) -> &str {
        &self.run
    }

    /// Returns the hosted run attempt.
    #[must_use]
    pub fn attempt(&self) -> &str {
        &self.attempt
    }

    /// Returns the source revision compiled into this fixture.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub(super) fn active(&self) -> io::Result<()> {
        let current = Self::open(&self.root)?;
        if current.run != self.run
            || current.attempt != self.attempt
            || current.revision != self.revision
            || current.root != self.root
        {
            return Err(refused());
        }
        Ok(())
    }
}

fn environment() -> io::Result<(String, String, String)> {
    if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
        || std::env::var("RUNNER_ENVIRONMENT").as_deref() != Ok("github-hosted")
        || std::env::var("RUNNER_OS").as_deref() != Ok("Windows")
    {
        return Err(refused());
    }
    let expected_arch = if cfg!(target_arch = "x86_64") {
        "X64"
    } else if cfg!(target_arch = "aarch64") {
        "ARM64"
    } else {
        return Err(refused());
    };
    if std::env::var("RUNNER_ARCH").as_deref() != Ok(expected_arch) {
        return Err(refused());
    }
    let run = std::env::var("GITHUB_RUN_ID").map_err(|_| refused())?;
    let attempt = std::env::var("GITHUB_RUN_ATTEMPT").map_err(|_| refused())?;
    if !positive_decimal(&run) || !positive_decimal(&attempt) {
        return Err(refused());
    }
    let revision = std::env::var("GITHUB_SHA").map_err(|_| refused())?;
    let built = option_env!("P4KX_FIXTURE_SOURCE_REVISION").ok_or_else(refused)?;
    if revision != built
        || revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refused());
    }
    let acknowledgement = format!("p4kx-native-registry-fixture:{run}:{attempt}");
    if std::env::var("P4KX_NATIVE_REGISTRY_FIXTURE").as_deref() != Ok(acknowledgement.as_str()) {
        return Err(refused());
    }
    Ok((run, attempt, revision))
}

fn positive_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 20
        && !value.starts_with('0')
        && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn checked_directory(path: &Path) -> io::Result<PathBuf> {
    use std::os::windows::fs::MetadataExt as _;
    const REPARSE_POINT: u32 = 0x400;
    if !path.is_absolute() {
        return Err(refused());
    }
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(refused());
        }
    }
    std::fs::canonicalize(path)
}

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "native Registry fixture requires an acknowledged disposable native hosted Windows runner",
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_identifiers_are_bounded_canonical_positive_decimals() {
        for value in ["", "0", "01", "+1", "1 ", "999999999999999999999"] {
            assert!(!super::positive_decimal(value));
        }
        for value in ["1", "42", "42360288384"] {
            assert!(super::positive_decimal(value));
        }
    }
}

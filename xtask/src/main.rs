// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Repository checks run by CI: `cargo xtask layers`, `cargo xtask lints`,
//! `cargo xtask file-size`, `cargo xtask changelog`, `cargo xtask keys`, `cargo xtask spdx`,
//! `cargo xtask toolchain` (exact compiler/test-runner pins), `cargo xtask addon` (the Blender add-on manifest version equals the workspace version),
//! `cargo xtask licences` (`LICENSE-MIT.md` and `LICENSE-APACHE.md` hold the words of the plain
//! licence files), or all of them with `cargo xtask`.
//!
//! `cargo xtask changelog` also checks the fragments in `changelog.d/`; `cargo xtask changelog
//! --release <version> [--date <YYYY-MM-DD>]` folds them and `## [Unreleased]` into the section of
//! a version.
//!
//! `cargo xtask notices` checks that `THIRD-PARTY-NOTICES.md` matches `cargo about` and
//! `cargo xtask notices --write` regenerates it; both need `cargo-about` and are not part of the
//! plain `cargo xtask`.
//!
//! `cargo xtask release-notes <tag> <file>` writes the `CHANGELOG.md` notes of a Release tag.

mod addon;
mod changelog;
mod file_size;
mod fragments;
mod key_needles;
mod keys;
mod layers;
mod licences;
mod lints;
mod metadata;
mod notices;
mod spdx;
mod toolchain;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

fn main() -> ExitCode {
    match run() {
        Ok(violations) if violations.is_empty() => ExitCode::SUCCESS,
        Ok(violations) => {
            report(&violations);
            ExitCode::FAILURE
        }
        Err(error) => {
            report(&[format!("error: {error:#}")]);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<Vec<String>> {
    let root = workspace_root()?;
    let command = std::env::args().nth(1);
    match command.as_deref() {
        None => check_all(&root),
        Some("spdx") => spdx::check(&root),
        Some("addon") => addon::check(&root),
        Some("layers") => layers::check(&root),
        Some("lints") => lints::check(&root),
        Some("toolchain") => toolchain::check(&root),
        Some("file-size") => file_size::check(&root),
        Some("changelog") => {
            let mut args = std::env::args().skip(2);
            match args.next().as_deref() {
                None => changelog::check(&root),
                Some("--release") => {
                    let Some(version) = args.next() else {
                        bail!(
                            "usage: cargo xtask changelog --release <version> [--date <YYYY-MM-DD>]"
                        );
                    };
                    let date = match (args.next().as_deref(), args.next()) {
                        (None, _) => fragments::today()?,
                        (Some("--date"), Some(date)) => date,
                        _ => bail!(
                            "usage: cargo xtask changelog --release <version> [--date <YYYY-MM-DD>]"
                        ),
                    };
                    fragments::release(&root, &version, &date)?;
                    Ok(Vec::new())
                }
                Some(other) => bail!("unknown option `{other}`; use `--release <version>`"),
            }
        }
        Some("licences") => licences::check(&root),
        Some("keys") => keys::check(&root),
        Some("notices") => match std::env::args().nth(2).as_deref() {
            None => notices::check(&root),
            Some("--write") => {
                notices::write(&root)?;
                Ok(Vec::new())
            }
            Some(other) => bail!("unknown option `{other}`; use `cargo xtask notices [--write]`"),
        },
        Some("release-notes") => {
            let mut args = std::env::args().skip(2);
            let (Some(tag), Some(output)) = (args.next(), args.next()) else {
                bail!("usage: cargo xtask release-notes <tag> <file>");
            };
            changelog::write_notes(&root, &tag, Path::new(&output))?;
            Ok(Vec::new())
        }
        Some(other) => bail!(
            "unknown check `{other}`; use `layers`, `lints`, `toolchain`, `file-size`, `changelog`, `keys`, `spdx`, `addon`, `licences`, `notices` or `release-notes`"
        ),
    }
}

fn check_all(root: &Path) -> Result<Vec<String>> {
    let mut violations = layers::check(root)?;
    violations.extend(lints::check(root)?);
    violations.extend(toolchain::check(root)?);
    violations.extend(file_size::check(root)?);
    violations.extend(changelog::check(root)?);
    violations.extend(keys::check(root)?);
    violations.extend(spdx::check(root)?);
    violations.extend(addon::check(root)?);
    violations.extend(licences::check(root)?);
    Ok(violations)
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(PathBuf::from)
        .context("xtask has no parent directory")
}

fn report(lines: &[String]) {
    let mut stderr = io::stderr().lock();
    for line in lines {
        let _ignored = writeln!(stderr, "{line}");
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::discovery_context::DiscoveryContext;
use crate::prefix_case;
use crate::prefix_fs::{DiscoveryPathKind, PrefixFileSystem};

/// The actual host path and observed target of one Wine drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WineDrive {
    letter: char,
    path: PathBuf,
    canonical: Option<PathBuf>,
}

/// One original interpretation context shared by all launcher users of a prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WineContext {
    pub(crate) prefix: PathBuf,
    c_drive: WineDrive,
    devices: PathBuf,
    drives: Vec<WineDrive>,
}

impl WineContext {
    pub(crate) fn discover<F: PrefixFileSystem>(
        context: &mut DiscoveryContext<'_, F>,
        prefix: &Path,
    ) -> Self {
        let c_drive = prefix_case::descend(context, prefix, &["drive_c"])
            .unwrap_or_else(|| prefix.join("drive_c"));
        let c_drive = WineDrive {
            letter: 'c',
            canonical: context.canonical(&c_drive),
            path: c_drive,
        };
        let devices = prefix_case::descend(context, prefix, &["dosdevices"])
            .unwrap_or_else(|| prefix.join("dosdevices"));
        let mut drives = Vec::new();
        if let Some(names) = context.entries(&devices) {
            for (letter, name) in drive_names(&names) {
                let path = devices.join(name);
                if !path.starts_with(&devices)
                    || context.kind(&path) != Some(DiscoveryPathKind::Directory)
                {
                    continue;
                }
                drives.push(WineDrive {
                    letter,
                    canonical: context.canonical(&path),
                    path,
                });
            }
        }

        Self {
            prefix: prefix.to_path_buf(),
            c_drive,
            devices,
            drives,
        }
    }

    pub(crate) fn c_drive(&self) -> &Path {
        &self.c_drive.path
    }

    pub(crate) fn host_path(&self, path: &Path) -> Option<PathBuf> {
        let (letter, rest) = crate::wine::split_drive(path.to_str()?)?;
        let drive = if letter == 'c' {
            &self.c_drive
        } else {
            self.drives.iter().find(|drive| drive.letter == letter)?
        };
        if letter != 'c' && !drive.path.starts_with(&self.devices) {
            return None;
        }
        let base = if drive.path.is_absolute() {
            drive.path.as_path()
        } else {
            drive
                .canonical
                .as_deref()
                .filter(|path| path.is_absolute())?
        };
        crate::wine::append_parts(base.to_path_buf(), rest)
            .filter(|path| path.is_absolute() && path.starts_with(base))
    }
}

fn drive_names(names: &[OsString]) -> BTreeMap<char, &OsStr> {
    let mut drives = BTreeMap::new();
    for name in names {
        let bytes = name.as_encoded_bytes();
        let Some(&letter) = bytes.first().filter(|letter| letter.is_ascii_alphabetic()) else {
            continue;
        };
        if bytes.len() != 2 || bytes.get(1) != Some(&b':') || letter.eq_ignore_ascii_case(&b'c') {
            continue;
        }
        let drive = char::from(letter.to_ascii_lowercase());
        let selected = drives.entry(drive).or_insert(name.as_os_str());
        if letter.is_ascii_lowercase() {
            *selected = name.as_os_str();
        }
    }
    drives
}

#[cfg(test)]
#[path = "wine_context_tests.rs"]
mod tests;

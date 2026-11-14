// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use p4kx_testkit::offline_seam;

use super::error::{Error, Kind};

const MAX_OUTPUT_BYTES: usize = 8 << 20;

pub(crate) struct Destination {
    path: PathBuf,
}

impl Destination {
    pub(crate) fn new(task_root: &Path, output: &Path) -> super::Result<Self> {
        if !task_root.is_absolute() || !output.is_absolute() {
            return Err(
                Error::new(Kind::Input, "metadata output requires absolute task paths").into(),
            );
        }
        let root = offline_seam::direct_root(task_root)?;
        let relative = output.strip_prefix(task_root).map_err(|_| {
            Error::new(
                Kind::Input,
                "metadata output is outside the explicit task root",
            )
        })?;
        let relative = relative
            .to_str()
            .ok_or_else(|| Error::new(Kind::Input, "metadata output path is not UTF-8"))?;
        let relative = offline_seam::relative_path(relative, 4_096)?;
        if !Path::new(&relative)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            return Err(Error::new(Kind::Input, "metadata output requires a JSON filename").into());
        }
        let path = root.join(relative);
        let parent = path
            .parent()
            .ok_or_else(|| Error::new(Kind::Input, "metadata output has no parent"))?;
        validate_parent(&root, parent)?;
        Ok(Self { path })
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> super::Result<()> {
        if bytes.len() > MAX_OUTPUT_BYTES {
            return Err(
                Error::new(Kind::Bounds, "metadata artifact exceeds its byte bound").into(),
            );
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
}

fn validate_parent(root: &Path, parent: &Path) -> super::Result<()> {
    let ancestors = parent.strip_prefix(root)?.components();
    let mut current = root.to_path_buf();
    for ancestor in ancestors {
        current.push(ancestor);
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.is_dir() || offline_seam::is_linked(&metadata) {
            return Err(Error::new(
                Kind::Input,
                "metadata output has a linked or non-directory ancestor",
            )
            .into());
        }
    }
    if !fs::canonicalize(parent)?.starts_with(root) {
        return Err(Error::new(Kind::Input, "metadata output escapes its task root").into());
    }
    Ok(())
}

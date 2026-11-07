// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::path::{Path, PathBuf};

use crate::package_fault::{self, Check};

pub(crate) const KEEP_VAR: &str = "P4KX_KEEP_PACKAGES";

pub(crate) struct Scratch {
    path: PathBuf,
    finished: Cell<bool>,
}

impl Scratch {
    pub(crate) fn new(test: &str) -> package_fault::Result<Self> {
        let path = std::env::temp_dir()
            .join("p4kx-package-verification")
            .join(test);
        std::fs::create_dir_all(&path)
            .map_err(|error| package_fault::Fault::at(Check::Scratch, error))?;
        Ok(Self {
            path,
            finished: Cell::new(false),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn finish(self, result: package_fault::Result<()>) -> package_fault::Result<()> {
        self.finished.set(true);
        if result.is_ok() && std::env::var_os(KEEP_VAR).is_none() {
            self.remove()?;
        }
        result
    }

    fn remove(&self) -> package_fault::Result<()> {
        std::fs::remove_dir_all(&self.path)
            .map_err(|error| package_fault::Fault::at(Check::Cleanup, error))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.finished.get() || std::thread::panicking() || std::env::var_os(KEEP_VAR).is_some() {
            return;
        }
        let result = self.remove();
        assert!(result.is_ok(), "package_cleanup_failed");
    }
}

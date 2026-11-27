// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process;

pub(crate) fn parent_of(out: &Path) -> PathBuf {
    out.parent().map_or_default(Path::to_path_buf)
}

pub(crate) fn sibling_name(name: &OsString, kind: &str) -> OsString {
    let mut sibling = name.clone();
    sibling.push(format!(".p4kx-{}.{kind}", process::id()));
    sibling
}

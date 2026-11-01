// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Path helper shared by the real-Archive tests.

/// Returns `path` in lower case with `/` as separator, the form Entry paths are compared in.
pub(crate) fn unified(path: &str) -> String {
    path.to_ascii_lowercase().replace('\\', "/")
}

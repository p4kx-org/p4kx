// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The parent picker failed without committing any target change.
#[derive(Debug)]
pub(crate) enum PickerFailure {
    Spawn(std::io::Error),
    Disconnected,
    RelativePath,
}

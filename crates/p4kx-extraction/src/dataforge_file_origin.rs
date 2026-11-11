// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "observed_file_origin.rs"]
mod observed_file_origin;

pub use observed_file_origin::ObservedFileOrigin;

/// The observed standalone input of a successfully read DataForge file.
pub type DataForgeFileOrigin = ObservedFileOrigin;

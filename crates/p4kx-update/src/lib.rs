// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Checks for newer Releases and selects native package links without downloading assets.
//! A single anonymous GitHub request sends no machine, profile or Archive information.

mod distribution;
mod platform;
mod release;
mod release_asset;
mod release_check;
mod update_error;

pub use platform::{
    Architecture, OperatingSystem, PackageKind, Platform, PlatformError, TargetDefinition,
    asset_filename, supported_targets,
};
pub use release::Release;
pub use release_asset::ReleaseAsset;
pub use release_check::ReleaseCheck;
pub use update_error::UpdateError;

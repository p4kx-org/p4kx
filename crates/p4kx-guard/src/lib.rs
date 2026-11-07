// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Keeps P4KX read-only towards the game: writes into a Star Citizen installation are
//! refused, and a running game is noticed from the process list alone.

mod archive_source_identity;
mod archive_write_protection;
#[cfg(any(target_os = "linux", test))]
mod bound_documents_reply;
mod created_output_file;
mod installation;
mod namespace_root;
mod namespace_target;
mod owned_directory_permit;
mod owned_file_token;
mod owned_output_cleanup;
mod owned_output_error;
mod portal_alias;
#[cfg(any(target_os = "linux", test))]
mod portal_dbus;
#[cfg(any(target_os = "linux", test))]
mod portal_deadline;
#[cfg(target_os = "linux")]
mod portal_filesystem;
#[cfg(any(target_os = "linux", test))]
mod portal_mount;
#[cfg(any(target_os = "linux", test))]
mod portal_mount_binding;
#[cfg(target_os = "linux")]
mod portal_native;
#[cfg(any(target_os = "linux", test))]
mod portal_path;
#[cfg(any(target_os = "linux", test))]
mod portal_reply;
#[cfg(any(target_os = "linux", test))]
mod portal_resolution;
#[cfg(target_os = "linux")]
mod portal_socket;
mod proc_tree;
mod process;
#[cfg(any(windows, test))]
mod process_windows;
mod registry;
mod runtime_prerequisite;
#[cfg(any(target_os = "linux", test))]
mod source_membership;
mod write_protection_error;
mod write_target;

use std::path::PathBuf;

pub use archive_write_protection::ArchiveWriteProtection;
pub use created_output_file::{CreatedOutputFile, create_output_file};
pub use installation::{Installation, without_verbatim_prefix};
pub use owned_directory_permit::{
    OwnedDirectoryPermit, create_output_directory, validated_output_directory,
};
pub use owned_file_token::OwnedFileToken;
pub use owned_output_cleanup::{
    OwnedOutputCleanupError, remove_owned_output, remove_owned_output_target_only,
};
pub use owned_output_error::OwnedOutputError;
pub use portal_alias::{AliasUncertainty, DocumentGrant, HostAlias, PortalAlias, UnknownAlias};
#[cfg(target_os = "linux")]
pub use portal_native::resolve_portal_alias;
pub use proc_tree::game_in_proc_tree;
pub use process::{
    any_game_process, game_is_running, game_process_status, is_game_process,
    process_count_lists_game, qprocess_lists_game,
};
pub use registry::{
    ensure_writable, protect, protect_archive_folder, protect_libraries, protect_manual,
};
pub use runtime_prerequisite::{
    RuntimeEnvironment, RuntimePrerequisiteError, check_runtime_environment,
    check_runtime_prerequisites,
};
pub use write_protection_error::WriteProtectionError;

/// A write that was refused because its target lies inside a Star Citizen installation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "{} lies inside the Star Citizen installation {}; the tool never writes there",
    target.display(),
    installation.display()
)]
pub struct WriteRefused {
    /// The target as given.
    pub target: PathBuf,
    /// The protected folder the target lies in.
    pub installation: PathBuf,
}

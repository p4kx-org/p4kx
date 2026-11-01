// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Keeps P4KX read-only towards the game: writes into a Star Citizen installation are
//! refused, and a running game is noticed from the process list alone.

mod installation;
mod proc_tree;
mod process;
mod registry;

use std::path::PathBuf;

pub use installation::{Installation, without_verbatim_prefix};
pub use proc_tree::game_in_proc_tree;
pub use process::{
    any_game_process, game_is_running, is_game_process, process_count_lists_game,
    qprocess_lists_game,
};
pub use registry::{
    ensure_writable, protect, protect_archive_folder, protect_libraries, protect_manual,
};

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

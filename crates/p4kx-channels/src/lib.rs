// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Finds the installed Channels of Star Citizen: in the default Library folder, in the Wine
//! prefixes of Linux, in the Library folders the RSI Launcher log names, and in manually added
//! folders and Archives.

mod channel;
mod detect;
mod kind;
mod launcher_log;
mod log_lines;
mod public_patch;
mod wine;

pub use channel::{Channel, ChannelSource, channel_name};
pub use detect::{SearchPaths, detect, detect_with_patches, known_library_folders};
pub use kind::ChannelKind;
pub use launcher_log::{LAUNCHER_LOG_FILES, launcher_logs_folder};
pub use public_patch::PublicPatches;
pub use wine::WineEnvironment;

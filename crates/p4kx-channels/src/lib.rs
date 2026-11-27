// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Finds the installed Channels of Star Citizen: in the default Library folder, in the Wine
//! prefixes of Linux, in the Library folders the RSI Launcher log names, and in manually added
//! folders and Archives.

mod discovery_context;
mod discovery_limits;
mod discovery_report;
mod launcher_read;
mod launcher_snapshot;
mod launcher_sources;
mod launcher_stamp;
mod opened_log;
mod ordered_libraries;
mod prefix_candidates;
mod prefix_case;
mod prefix_discovery;
mod prefix_fs;
mod prefix_seed_walk;
mod search_recipe;
mod steam_config;
mod steam_libraries;
mod steam_tokens;
mod wine_context;
mod wine_environment;

mod channel;
mod detect;
mod kind;
mod launcher_log;
mod log_lines;
mod public_patch;
mod wine;

pub use channel::{Channel, ChannelSource, channel_name};
pub use detect::{
    DetectedChannels, SearchPaths, detect, detect_with_patches, known_library_folders,
};
pub use kind::ChannelKind;
pub use launcher_log::{LAUNCHER_LOG_FILES, launcher_logs_folder};
pub use public_patch::PublicPatches;
pub use wine::WineEnvironment;

pub use discovery_limits::{DiscoveryLimit, DiscoveryLimits};
pub use discovery_report::{DiscoveryIssue, DiscoveryIssueKind, DiscoveryReport};
pub use launcher_snapshot::LauncherSnapshot;
pub use launcher_stamp::LauncherStamp;
pub use prefix_candidates::PrefixSeeds;
pub use prefix_fs::{
    DiscoveryPathKind, NativeDirectoryEntries, NativePrefixFileSystem, PrefixFileSystem,
};
pub use search_recipe::SearchRecipe;

#[cfg(test)]
mod launcher_test_support;

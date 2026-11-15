// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::MAX_SOUNDBANK_LEN;
use p4kx_extraction::{AudioError, Bank};

use crate::archives::Opened;

use crate::audio_wem_shapes::WemShape;
use crate::error::ToolError;

#[path = "audio_wem_info.rs"]
mod info;
pub(crate) use info::info;

#[path = "audio_wem_list.rs"]
mod list;
pub(crate) use list::soundbank_wems;

#[path = "audio_wem_payload_origins.rs"]
pub(crate) mod payload_origins;
pub(crate) use payload_origins::EvidenceError;

/// A WEM that cannot be parsed as a sound.
fn unreadable(id: Option<u32>, size: u64, error: Option<&AudioError>) -> WemShape {
    let plugin = matches!(error, Some(AudioError::PluginMedia));
    WemShape {
        id,
        size,
        prefetch: error.is_none(),
        codec: None,
        codec_name: None,
        channels: None,
        sample_rate: None,
        sample_count: None,
        duration_seconds: None,
        looping: None,
        plugin_media: plugin,
        error: error.filter(|_| !plugin).map(ToString::to_string),
    }
}

/// Reads the soundbank Entry at `path`, refusing one larger than the audio index reads.
pub(crate) fn open_bank(opened: &Opened, path: &str) -> Result<Bank, ToolError> {
    if let Some(entry) = opened.index().find(path)
        && entry.uncompressed_size() > MAX_SOUNDBANK_LEN
    {
        return Err(ToolError::SoundbankTooLarge {
            path: entry.path().to_owned(),
            limit: MAX_SOUNDBANK_LEN,
        });
    }
    Bank::from_archive_observed(opened.archive(), path)
        .map_err(|error| ToolError::converted(path, "a soundbank", error))
}

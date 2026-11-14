// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::MAX_SOUNDBANK_LEN;
use p4kx_extraction::{AudioError, Bank, EmbeddedWem};

use crate::archives::Opened;

use crate::audio_params::{MAX_NAME_LEN, SoundbankParams, WemParams};
use crate::audio_shapes::LocationShape;
use crate::audio_wem_input::{Held, WemOps, candidates, read};
use crate::audio_wem_shapes::{SoundbankWemsResult, WemInfoResult, WemShape};

use crate::budget::{fill_within, json_len};
use crate::error::{ToolError, capped};
use crate::page::Page;

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

fn embedded_shape(wem: &EmbeddedWem<'_>) -> WemShape {
    let size = wem.bytes().len() as u64;
    let id = Some(wem.id());
    if wem.is_prefetch() {
        return unreadable(id, size, None);
    }
    match wem.parse() {
        Ok(parsed) => parsed.shape(id, size),
        Err(error) => unreadable(id, size, Some(&error)),
    }
}

/// Returns one page of the WEMs the soundbank at `params.soundbank` embeds, each described by its
/// header.
pub(crate) fn soundbank_wems(
    opened: &Opened,
    params: &SoundbankParams,
    page: Page,
) -> Result<SoundbankWemsResult, ToolError> {
    let path = capped("soundbank", &params.soundbank, MAX_NAME_LEN * 4)?;
    let bank = open_bank(opened, path)?;
    let soundbank = bank.soundbank();
    let wems: Vec<EmbeddedWem<'_>> = soundbank.wems().collect();
    let items = page.slice(&wems).iter().map(embedded_shape);
    let mut result = SoundbankWemsResult {
        archive: opened.archive().path().display().to_string(),
        soundbank: bank.name().to_owned(),
        version: soundbank.header().version,
        bank_id: soundbank.header().id,
        total: wems.len(),
        wems: Vec::new(),
        next_cursor: None,
    };
    let (listed, next_cursor) = fill_within(
        page,
        wems.len(),
        json_len(&result),
        items.map(Ok),
        |wem: &WemShape| format!("WEM {}", wem.id.unwrap_or_default()),
    )?;
    result.wems = listed;
    result.next_cursor = next_cursor;
    Ok(result)
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

/// Describes the WEM `params` names from the first place that holds all of it.
pub(crate) fn info(
    opened: &Opened,
    params: &WemParams,
    page: Page,
) -> Result<WemInfoResult, ToolError> {
    let (id, places) = candidates(opened, params.wem_id, params.path.as_deref())?;
    page.check(places.len())?;
    let mut first: Option<(LocationShape, WemShape)> = None;
    let mut failure: Option<ToolError> = None;
    for place in &places {
        let size = place.location.len;
        let outcome = read(opened, place, |held| {
            Ok(match held {
                Held::Parsed(parsed) => (parsed.wem.shape(id, size), true),
                Held::Prefetch => (unreadable(id, size, None), false),
                Held::Unreadable(error) => (unreadable(id, size, Some(&error)), false),
            })
        });
        match outcome {
            Ok((shape, true)) => {
                first = Some((place.location.clone(), shape));
                break;
            }
            Ok((shape, false)) => {
                first.get_or_insert_with(|| (place.location.clone(), shape));
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    let Some((described, wem)) = first else {
        return Err(failure.unwrap_or(ToolError::WemSelector));
    };
    let total = places.len();
    let mut result = WemInfoResult {
        archive: opened.archive().path().display().to_string(),
        wem_id: id,
        locations_total: total,
        locations: Vec::new(),
        next_cursor: None,
        described,
        wem,
    };
    let (listed, next_cursor) = fill_within(
        page,
        total,
        json_len(&result),
        places
            .into_iter()
            .skip(page.offset())
            .map(|place| Ok(place.location)),
        |location: &LocationShape| format!("location {}", location.path),
    )?;
    result.locations = listed;
    result.next_cursor = next_cursor;
    Ok(result)
}

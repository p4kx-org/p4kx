// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{AudioError, EmbeddedWem, Wem};

use crate::archives::Opened;
use crate::audio_params::{MAX_NAME_LEN, SoundbankParams};
use crate::audio_wem::EvidenceError;
use crate::audio_wem_input::WemOps as _;
use crate::audio_wem_shapes::{SoundbankWemsResult, WemShape};
use crate::budget::fit_prefix;
use crate::error::{ToolError, capped};
use crate::page::Page;

#[path = "audio_wem_list_origins.rs"]
mod origins;

/// Returns one page of embedded WEM headers and their original source fields.
pub(crate) fn soundbank_wems(
    opened: &Opened,
    params: &SoundbankParams,
    page: Page,
) -> Result<SoundbankWemsResult, ToolError> {
    let path = capped("soundbank", &params.soundbank, MAX_NAME_LEN * 4)?;
    let bank = super::open_bank(opened, path)?;
    let soundbank = bank.soundbank();
    let wems = soundbank.wems();
    let total = wems.len();
    page.check(total)?;
    let candidates: Vec<_> = wems
        .skip(page.offset())
        .take(page.limit())
        .map(ListedWem::read)
        .collect();
    let response = |count| {
        let rows = candidates
            .get(..count)
            .ok_or(EvidenceError::InvalidPrefix)?;
        let next_cursor = page.next_cursor_after(count, total)?;
        let (field_origins, provenance) = origins::capture(&bank, rows, next_cursor.as_deref())?;
        Ok(SoundbankWemsResult {
            archive: opened.archive().path().display().to_string(),
            soundbank: bank.name().to_owned(),
            version: soundbank.header().version,
            bank_id: soundbank.header().id,
            total,
            wems: rows.iter().map(ListedWem::shape).collect(),
            next_cursor,
            field_origins,
            provenance,
        })
    };
    let (_, result) = fit_prefix(candidates.len(), response, || {
        candidates.first().map_or_else(
            || "soundbank WEM listing".to_owned(),
            |row| format!("WEM {}", row.wem.id()),
        )
    })?;
    Ok(result)
}

struct ListedWem<'a> {
    wem: EmbeddedWem<'a>,
    outcome: Outcome<'a>,
}

enum Outcome<'a> {
    Prefetch,
    Parsed(Wem<&'a [u8]>),
    Unreadable(AudioError),
}

impl<'a> ListedWem<'a> {
    fn read(wem: EmbeddedWem<'a>) -> Self {
        let outcome = if wem.is_prefetch() {
            Outcome::Prefetch
        } else {
            match wem.parse() {
                Ok(parsed) => Outcome::Parsed(parsed),
                Err(error) => Outcome::Unreadable(error),
            }
        };
        Self { wem, outcome }
    }

    fn shape(&self) -> WemShape {
        let id = Some(self.wem.id());
        let size = self.wem.bytes().len() as u64;
        match &self.outcome {
            Outcome::Prefetch => super::unreadable(id, size, None),
            Outcome::Parsed(parsed) => parsed.shape(id, size),
            Outcome::Unreadable(error) => super::unreadable(id, size, Some(error)),
        }
    }
}

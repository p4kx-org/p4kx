// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank_wem_evidence::{BankWemRead, BankWemReadMode};

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
        .map(|wem| BankWemRead::read(wem, BankWemReadMode::AllNonPrefetched))
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
            wems: rows.iter().map(shape).collect(),
            next_cursor,
            field_origins,
            provenance,
        })
    };
    let (_, result) = fit_prefix(candidates.len(), response, || {
        candidates.first().map_or_else(
            || "soundbank WEM listing".to_owned(),
            |row| format!("WEM {}", row.media().id()),
        )
    })?;
    Ok(result)
}

fn shape(row: &BankWemRead<'_>) -> WemShape {
    let id = Some(row.media().id());
    let size = row.media().bytes().len() as u64;
    row.parsed().map_or_else(
        || super::unreadable(id, size, row.error()),
        |parsed| parsed.shape(id, size),
    )
}

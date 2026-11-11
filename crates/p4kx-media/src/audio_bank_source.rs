// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::SectionId;
use p4kx_extraction::bank::{BankSource, BankSourceSection};

use crate::audio_bank_failure::BankPhase;
use crate::audio_bank_inventory::BankInventory;
use crate::audio_bank_projection::BankProjection;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, audio_bank_hirc, audio_bank_media, audio_bank_stid};

pub(crate) fn project<R: Read + Seek>(
    input: &MediaOriginInput,
    path: &str,
    length: u64,
    reader: R,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let projection = BankProjection {
        input,
        path,
        length,
    };
    let mut source = match BankSource::open(reader, length) {
        Ok(source) => source,
        Err(cause) => return projection.failure(BankPhase::Signature, None, cause, writer),
    };
    let inventory = directory(&projection, &mut source, writer)?;
    subresources(&projection, &mut source, &inventory, writer)
}

fn directory<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    writer: &mut MediaWriter<'_>,
) -> Result<BankInventory, MediaError> {
    let mut inventory = BankInventory::new();
    let mut cursor = source.section_cursor();
    while let Some(result) = source.next_section(&mut cursor) {
        let section = match result {
            Ok(section) => section,
            Err(cause) => {
                projection.failure(BankPhase::Directory, None, cause, writer)?;
                inventory.complete = false;
                inventory.hirc_objects.mark_incomplete();
                inventory.didx_entries.mark_incomplete();
                break;
            }
        };
        projection.section(&section, writer)?;
        inventory.seen_section(&section);
        match section.id() {
            SectionId::BKHD => {
                observe_header(projection, source, &section, &mut inventory, writer)?;
            }
            SectionId::DATA => inventory.observe_data(section)?,
            SectionId::DIDX => inventory
                .didx_entries
                .observe_didx(projection, source, &section)?,
            SectionId::HIRC => inventory
                .hirc_objects
                .observe_hirc(projection, source, &section)?,
            _ => {}
        }
    }
    Ok(inventory)
}

fn observe_header<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    section: &BankSourceSection,
    inventory: &mut BankInventory,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    match source.read_observed_header_for(section) {
        Ok(header) => {
            let values = header.header();
            inventory.header(Some(header))?;
            projection.header(section, &values, writer)
        }
        Err(cause) => {
            inventory.header(None)?;
            projection.failure(BankPhase::Header, Some(section), cause, writer)
        }
    }
}

fn subresources<R: Read + Seek>(
    projection: &BankProjection<'_>,
    source: &mut BankSource<R>,
    inventory: &BankInventory,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Some(last_section) = inventory.last_child_section else {
        return Ok(());
    };
    let mut cursor = source.section_cursor();
    for _ in 0..=last_section {
        let section = source
            .next_section(&mut cursor)
            .ok_or_else(|| {
                MediaError::Contract("retained BNK directory ended during its second pass".into())
            })?
            .map_err(|cause| projection.replay_failure(cause))?;
        match section.id() {
            SectionId::DIDX => {
                audio_bank_media::project(projection, source, &section, inventory.data(), writer)?;
            }
            SectionId::HIRC => {
                audio_bank_hirc::project(projection, source, &section, inventory, writer)?;
            }
            SectionId::STID => audio_bank_stid::project(projection, source, &section, writer)?,
            _ => {}
        }
    }
    Ok(())
}

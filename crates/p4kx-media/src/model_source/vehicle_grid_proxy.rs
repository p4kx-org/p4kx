// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::path::Path;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{Chunk, ChunkDirectory, ChunkDirectoryError, ChunkError, ivo};
use p4kx_extraction::model::StatObjPhysics;

use super::{GridCollisionState, merge};
use crate::origin_contributions::OriginContributions;
use crate::provider::model_source::origins::ModelOrigins;
use crate::{Identity, MediaError, MediaWriter, RequiredInput, error};

pub(super) struct Proxy {
    pub(super) state: GridCollisionState,
    pub(super) evidence: OriginContributions,
    pub(super) tables: Vec<Table>,
}

pub(super) struct Table {
    pub(super) ordinal: u64,
    pub(super) evidence: OriginContributions,
    pub(super) slots: Vec<Slot>,
}

pub(super) struct Slot {
    pub(super) ordinal: u64,
    pub(super) evidence: OriginContributions,
}

pub(super) fn read(
    reader: &mut RequiredInput<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Proxy, MediaError> {
    let source = &reader.receipt().source;
    let length = source.size;
    let entry = receipt_entry(reader)?.entry_index;
    let input = reader.origin_input(writer)?;
    let origins = ModelOrigins {
        input: &input,
        entry,
    };
    match reader.read_chunk_directory() {
        Ok(file) => inspect(&file, &origins, reader, writer),
        Err(ChunkDirectoryError::Format(ChunkError::Allocation)) => Err(MediaError::Contract(
            "cannot allocate admitted vehicle grid proxy directory".into(),
        )),
        Err(ChunkDirectoryError::Format(_)) => malformed(length.min(16), None, &origins, writer),
        Err(ChunkDirectoryError::InvalidRow { range, .. }) => {
            malformed(length.min(16), Some(range), &origins, writer)
        }
        Err(ChunkDirectoryError::Input(cause)) => {
            Err(error::io(Path::new(&receipt_entry(reader)?.path), cause))
        }
        Err(_) => Err(MediaError::Contract(
            "unsupported vehicle grid proxy directory failure".into(),
        )),
    }
}

fn receipt_entry<'a>(
    reader: &'a RequiredInput<'_>,
) -> Result<&'a crate::ContainerIdentity, MediaError> {
    match &reader.receipt().source.identity {
        Identity::Entry { entry, .. } => Ok(entry),
        _ => Err(MediaError::Contract(
            "vehicle grid proxy has no physical Entry".into(),
        )),
    }
}

fn inspect(
    file: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    reader: &mut RequiredInput<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Proxy, MediaError> {
    let table = origins.payload(
        file.table_range(),
        "modelVehicleGridProxyDirectory",
        SourceUseRole::Selector,
        writer,
    )?;
    let header = origins.payload(
        0..16,
        "modelVehicleGridProxyHeader",
        SourceUseRole::Selector,
        writer,
    )?;
    let evidence = merge(&header, &table, writer)?;
    let mut tables = Vec::new();
    for (ordinal, chunk) in file.chunks().iter().enumerate() {
        writer.checkpoint()?;
        if chunk.chunk_type() == ivo::STAT_OBJ_PHYSICS {
            let table = read_table(file, chunk, ordinal, origins, reader, writer)?;
            reserve(&mut tables, 1)?;
            tables.push(table);
        }
    }
    let state = if tables.is_empty() {
        GridCollisionState::NoPhysicsTables
    } else {
        GridCollisionState::TablesPresent
    };
    Ok(Proxy {
        state,
        evidence,
        tables,
    })
}

fn malformed(
    header_end: u64,
    row: Option<Range<u64>>,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Proxy, MediaError> {
    let mut evidence = origins.payload(
        0..header_end,
        "modelVehicleGridProxyInvalidHeader",
        SourceUseRole::Selector,
        writer,
    )?;
    if let Some(row) = row {
        let failed = origins.payload(
            row,
            "modelVehicleGridProxyInvalidRow",
            SourceUseRole::Selector,
            writer,
        )?;
        evidence = merge(&evidence, &failed, writer)?;
    }
    Ok(Proxy {
        state: GridCollisionState::MalformedTarget,
        evidence,
        tables: Vec::new(),
    })
}

fn read_table(
    file: &ChunkDirectory,
    chunk: &Chunk,
    ordinal: usize,
    origins: &ModelOrigins<'_>,
    input: &mut RequiredInput<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Table, MediaError> {
    let row = file.table_row(ordinal).ok_or_else(|| {
        MediaError::Contract("vehicle grid proxy lost a physical chunk table row".into())
    })?;
    let kind = origins.payload(row.kind, "modelChunkType", SourceUseRole::Selector, writer)?;
    let version = origins.payload(
        row.version,
        "modelChunkVersion",
        SourceUseRole::Selector,
        writer,
    )?;
    let evidence = merge(&kind, &version, writer)?;
    let mut table = Table {
        ordinal: ordinal as u64,
        evidence,
        slots: Vec::new(),
    };
    if StatObjPhysics::require_layout(chunk, ordinal).is_err() {
        return Ok(table);
    }
    let extent = crate::provider::model_source::origins::chunk_extent(
        file.table_range(),
        row.offset,
        row.size,
        origins,
        writer,
    )?;
    let start = chunk.offset();
    let bytes = input.read_chunk(start..start + chunk.size())?;
    let Ok(physics) = StatObjPhysics::read(&bytes) else {
        return Ok(table);
    };
    let count = global(start, StatObjPhysics::slot_count_range());
    let count = origins.payload(count, "physicsSlotCount", SourceUseRole::Selector, writer)?;
    table.evidence = merge(&table.evidence, &count, writer)?;
    for slot in physics.slots() {
        input.charge_metadata_work(1)?;
        let positions = slot.length_range().start..slot.key_range().end;
        let original = origins.payload(
            global(start, positions),
            "physicsSlotTableRow",
            SourceUseRole::Selector,
            writer,
        )?;
        let evidence = merge(&original, &extent.start, writer)?;
        reserve(&mut table.slots, 1)?;
        table.slots.push(Slot {
            ordinal: slot.ordinal() as u64,
            evidence,
        });
    }
    Ok(table)
}

const fn global(base: u64, range: Range<u64>) -> Range<u64> {
    base + range.start..base + range.end
}

fn reserve<T>(items: &mut Vec<T>, additional: usize) -> Result<(), MediaError> {
    items.try_reserve(additional).map_err(|cause| {
        MediaError::Contract(format!(
            "cannot retain vehicle grid collision locators: {cause}"
        ))
    })
}

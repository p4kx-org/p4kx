// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{FlatValues, PositionedRecord};

use super::field_receipt::RecordOriginal;
use super::flat_fields;
use super::record_stream_input::{RecordStreamInput, foreign};
use crate::resolved_texts::LocalizationLookupEvidence;
use crate::{
    DataForgeOrigin, DecimalU64, FieldPath, OriginCompanionSpool, OriginOutputAddress,
    RecordJsonLocation, RecordLocalization, StagedOriginCompanion,
};

/// Original fields observed while writing the streamed rows of one Record.
pub struct RecordStreamRows<'input, 'locale> {
    input: &'input DataForgeOrigin,
    fields: Vec<StreamField<'locale>>,
    count: u64,
}

impl<'input, 'locale> RecordStreamRows<'input, 'locale> {
    /// Writes flat rows and retains the original fields behind the emitted columns.
    ///
    /// # Errors
    /// Returns a foreign evidence, positioning, serialization or allocation error.
    pub fn write_flat(
        input: RecordStreamInput<'input, '_>,
        writer: &mut FlatValues,
        record: &PositionedRecord<'_>,
        out: &mut Vec<u8>,
        locale: Option<&RecordLocalization<'locale>>,
    ) -> io::Result<Self> {
        input.check(record)?;
        let (count, fields) = flat_fields::write_flat(input.forge(), writer, record, out, locale)?;
        Ok(Self::new(input, count, fields))
    }

    pub(super) const fn new(
        input: RecordStreamInput<'input, '_>,
        count: u64,
        fields: Vec<StreamField<'locale>>,
    ) -> Self {
        Self {
            input: input.origin(),
            fields,
            count,
        }
    }
}

pub(super) struct StreamField<'locale> {
    pub(super) address: StreamAddress,
    pub(super) sites: Vec<Option<RecordJsonLocation>>,
    pub(super) complete: bool,
    pub(super) lookup: Option<StreamLookup<'locale>>,
}

pub(super) struct StreamLookup<'locale> {
    pub(super) authored_key: Option<RecordJsonLocation>,
    pub(super) evidence: Option<LocalizationLookupEvidence<'locale>>,
}

pub(super) enum StreamAddress {
    Json {
        row: u64,
        path: FieldPath,
    },
    Text {
        row: u64,
        column: &'static str,
        start: u64,
        end: u64,
    },
}

impl StreamAddress {
    fn at(self, row_base: u64, part_start: u64) -> io::Result<OriginOutputAddress> {
        match self {
            Self::Json { row, path } => Ok(OriginOutputAddress::NdjsonField {
                line: DecimalU64::new(add(row_base, row)?),
                path,
            }),
            Self::Text {
                row,
                column,
                start,
                end,
            } => {
                let index = add(row_base, row)?;
                Ok(OriginOutputAddress::TextBytes {
                    start: DecimalU64::new(add(part_start, start)?),
                    end_exclusive: DecimalU64::new(add(part_start, end)?),
                    field: FieldPath::parse(&format!("/rows/{index}/{column}"))
                        .map_err(io::Error::other)?,
                })
            }
        }
    }
}

/// Stages proven fields from emitted Records in the order their output commits.
pub struct RecordStreamSpool<'input> {
    input: &'input DataForgeOrigin,
    original: RecordOriginal,
    spool: OriginCompanionSpool,
    row_count: u64,
}

impl<'input> RecordStreamSpool<'input> {
    /// Starts a bounded companion spool for one exact DataForge input.
    ///
    /// # Errors
    /// Returns an original evidence or temporary stream error.
    pub fn new(input: RecordStreamInput<'input, '_>) -> io::Result<Self> {
        let spool = OriginCompanionSpool::new()?;
        let original = RecordOriginal::observe(spool.sources(), input.origin())?;
        Ok(Self {
            input: input.origin(),
            original,
            spool,
            row_count: 0,
        })
    }

    /// Stages the successfully written Records of one part at its artifact byte offset.
    ///
    /// # Errors
    /// Returns a foreign evidence, coordinate overflow or staging error.
    pub fn append_part(
        &mut self,
        rows: Vec<RecordStreamRows<'input, '_>>,
        part_start: u64,
    ) -> io::Result<()> {
        if rows.iter().any(|row| !std::ptr::eq(row.input, self.input)) {
            return Err(foreign());
        }
        for record in rows {
            let next = add(self.row_count, record.count)?;
            for field in record.fields {
                let address = field.address.at(self.row_count, part_start)?;
                let (sources, complete) = match field.lookup {
                    Some(lookup) => self.original.bind_lookup(
                        field.sites,
                        lookup.authored_key.as_ref(),
                        field.complete,
                        lookup.evidence.as_ref(),
                        self.spool.sources(),
                    )?,
                    None => self.original.bind(field.sites, field.complete)?,
                };
                self.spool.push(&address, &sources, complete)?;
            }
            self.row_count = next;
        }
        self.spool.finish_batch()
    }

    /// Completes the companion for publication with the measured artifact.
    ///
    /// # Errors
    /// Returns a source evidence or temporary stream error.
    pub fn finish(self) -> io::Result<StagedOriginCompanion> {
        self.spool.finish()
    }
}

fn add(base: u64, offset: u64) -> io::Result<u64> {
    base.checked_add(offset)
        .ok_or_else(|| io::Error::other("Record stream position exceeds u64"))
}

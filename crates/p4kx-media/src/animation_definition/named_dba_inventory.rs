// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::model::DbaMetadataRecords;
use p4kx_extraction::{DecimalU64, InputScope, SourceUseRole};

use super::super::super::links;
use super::super::super::projection::DefinitionProjection;
use super::super::ClipCandidate;
use super::allocation;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{Identity, MediaError, MediaWriter, OffsetBasis, RequiredInput, SourceRange};

pub(in super::super) struct Inventory {
    pub(super) complete: bool,
    pub(super) coverage: OriginContributions,
    input: MediaOriginInput,
    identity: Identity,
    records: HashMap<String, Vec<Record>>,
}

impl Inventory {
    pub(super) fn matches(&self, path: &str) -> impl Iterator<Item = ObservedMatch<'_>> {
        self.records
            .get(path)
            .into_iter()
            .flatten()
            .map(move |record| ObservedMatch { bank: self, record })
    }
}
struct Record {
    ordinal: usize,
    authored_name: String,
    name: Range<u64>,
    metadata: Range<u64>,
}

pub(in super::super) struct ObservedMatch<'a> {
    bank: &'a Inventory,
    record: &'a Record,
}

pub(in super::super) struct MatchProof {
    pub(in super::super) entry: OriginContributions,
    pub(in super::super) name: OriginContributions,
    pub(in super::super) metadata: OriginContributions,
}

impl<'a> ObservedMatch<'a> {
    pub(in super::super) fn candidate(&self, declaration: u64) -> ClipCandidate<'a> {
        ClipCandidate {
            declaration_ordinal: DecimalU64::new(declaration),
            entry: &self.bank.identity,
            clip_ordinal: DecimalU64::new(self.record.ordinal as u64),
            authored_name: &self.record.authored_name,
            name_range: entry_range(self.record.name.clone()),
            metadata_range: entry_range(self.record.metadata.clone()),
        }
    }

    pub(in super::super) fn proof(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<MatchProof, MediaError> {
        let mut entry = writer.capture_origins();
        let input = &self.bank.input;
        entry.add(
            input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[], "reference-candidate"),
            SourceUseRole::Selector,
            writer,
        )?;
        let evidence = |range: &Range<u64>, field| {
            let mut proof = writer.capture_origins();
            proof.add(
                input,
                InputScope::Payload,
                OriginSite::new("animation-byte-range", &[range.start, range.end], field),
                SourceUseRole::Selector,
                writer,
            )?;
            Ok::<_, MediaError>(proof.finish())
        };
        Ok(MatchProof {
            entry: entry.finish(),
            name: evidence(&self.record.name, "databaseName")?,
            metadata: evidence(&self.record.metadata, "clipMetadata")?,
        })
    }
}

const fn entry_range(range: Range<u64>) -> SourceRange {
    SourceRange {
        basis: OffsetBasis::Entry,
        start: range.start,
        end_exclusive: range.end,
    }
}

pub(super) fn read(
    input: &mut RequiredInput<'_>,
    projection: &DefinitionProjection<'_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<Inventory, MediaError> {
    let bytes = input.read_payload()?;
    let origin = input.origin_input(writer)?;
    let mut proof = writer.capture_origins();
    let (records, complete, evidence_range) = match ChunkFile::parse(bytes.as_slice()) {
        Ok(file) => {
            let evidence_range = file.chunks_of(ivo::DBA_METADATA).next().map_or_else(
                || file.table_range(),
                |(_, chunk)| chunk.offset()..chunk.offset() + chunk.size(),
            );
            let (records, complete) = metadata(&file, projection, writer)?;
            (records, complete, evidence_range)
        }
        Err(_) => (HashMap::new(), false, 0..input.receipt().source.size),
    };
    proof.add(
        &origin,
        InputScope::Payload,
        OriginSite::new(
            "animation-byte-range",
            &[evidence_range.start, evidence_range.end],
            "dbaMetadataInventory",
        ),
        SourceUseRole::Selector,
        writer,
    )?;
    Ok(Inventory {
        complete,
        coverage: proof.finish(),
        input: origin,
        identity: input.receipt().source.identity.clone(),
        records,
    })
}

type Records = HashMap<String, Vec<Record>>;

fn metadata(
    file: &ChunkFile<&[u8]>,
    projection: &DefinitionProjection<'_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<(Records, bool), MediaError> {
    let budget = &projection.context.directory.input_state()?.budget;
    let names = DbaMetadataRecords::read(projection.context.entry.path(), file);
    let mut complete = names.is_ok();
    let mut records = Records::new();
    for name in names.into_iter().flatten() {
        budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
        let Ok(name) = name else {
            complete = false;
            break;
        };
        let Some((authored, key)) = std::str::from_utf8(name.name.bytes())
            .ok()
            .and_then(|name| {
                links::archive_animation_path(name).map(|path| (name, path.to_ascii_lowercase()))
            })
        else {
            complete = false;
            continue;
        };
        records
            .try_reserve(1)
            .map_err(|error| allocation(projection, error))?;
        let matches = records.entry(key).or_default();
        matches
            .try_reserve(1)
            .map_err(|error| allocation(projection, error))?;
        matches.push(Record {
            ordinal: name.ordinal,
            authored_name: authored.to_owned(),
            name: name.name.range(),
            metadata: name.metadata.range(),
        });
    }
    Ok((records, complete))
}

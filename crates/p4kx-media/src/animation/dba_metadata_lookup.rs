// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::model::DbaMetadataRecords;
use p4kx_extraction::{InputScope, SourceUseRole};

use super::selection_path::archive_animation_path;
use crate::SourceContext;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{Identity, MediaError, MediaWriter, OffsetBasis, RequiredInput, SourceRange};

pub(crate) struct Inventory {
    pub(crate) complete: bool,
    pub(crate) coverage: OriginContributions,
    input: MediaOriginInput,
    pub(crate) identity: Identity,
    records: HashMap<String, Vec<Record>>,
}

impl Inventory {
    pub(crate) fn matches(&self, path: &str) -> impl Iterator<Item = ObservedMatch<'_>> {
        self.records
            .get(path)
            .into_iter()
            .flatten()
            .map(move |record| ObservedMatch { bank: self, record })
    }
}
pub(crate) struct Record {
    pub(crate) ordinal: usize,
    pub(crate) authored_name: String,
    pub(crate) name: Range<u64>,
    pub(crate) metadata: Range<u64>,
}

pub(crate) struct ObservedMatch<'a> {
    pub(crate) bank: &'a Inventory,
    pub(crate) record: &'a Record,
}

pub(crate) struct MatchProof {
    pub(crate) entry: OriginContributions,
    pub(crate) name: OriginContributions,
    pub(crate) metadata: OriginContributions,
}

impl ObservedMatch<'_> {
    pub(crate) fn proof(&self, writer: &MediaWriter<'_>) -> Result<MatchProof, MediaError> {
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

pub(crate) const fn entry_range(range: Range<u64>) -> SourceRange {
    SourceRange {
        basis: OffsetBasis::Entry,
        start: range.start,
        end_exclusive: range.end,
    }
}

pub(crate) fn read(
    input: &mut RequiredInput<'_>,
    context: &SourceContext<'_>,
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
            let (records, complete) = metadata(&file, context, writer)?;
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

/// Matches exact normalized archive paths while retaining the authored spelling.
pub(crate) fn name_key(bytes: &[u8]) -> Option<(&str, String)> {
    let authored = std::str::from_utf8(bytes).ok()?;
    let key = archive_animation_path(authored)?.to_ascii_lowercase();
    Some((authored, key))
}

type Records = HashMap<String, Vec<Record>>;

fn metadata(
    file: &ChunkFile<&[u8]>,
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(Records, bool), MediaError> {
    let budget = &context.directory.input_state()?.budget;
    let names = DbaMetadataRecords::read(context.entry.path(), file);
    let mut complete = names.is_ok();
    let mut records = Records::new();
    for name in names.into_iter().flatten() {
        budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
        let Ok(name) = name else {
            complete = false;
            break;
        };
        let Some((authored, key)) = name_key(name.name.bytes()) else {
            complete = false;
            continue;
        };
        records
            .try_reserve(1)
            .map_err(|error| allocation(context, error))?;
        let matches = records.entry(key).or_default();
        matches
            .try_reserve(1)
            .map_err(|error| allocation(context, error))?;
        matches.push(Record {
            ordinal: name.ordinal,
            authored_name: authored.to_owned(),
            name: name.name.range(),
            metadata: name.metadata.range(),
        });
    }
    Ok((records, complete))
}

fn allocation(context: &SourceContext<'_>, cause: std::collections::TryReserveError) -> MediaError {
    MediaError::Io {
        path: context.entry.path().into(),
        source: std::io::Error::new(std::io::ErrorKind::OutOfMemory, cause),
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;

use p4kx_extraction::{Archive, OriginReaderLimits, SourceEncryption};
use serde::Deserialize;
use serde::de::DeserializeSeed as _;

use crate::audio_control_payload as wire;
use crate::origin_contract::{OriginProfile, OriginStream, OriginStreamSummary};
use crate::origin_coverage_read::Coverage;
use crate::origin_inventory_wire::{Encryption, Role, Scope, SourceKind};
use crate::origin_rows_read::{ReadOrigin, ReadRow, RowSeed};
use crate::{MediaError, MediaKind, MediaRecord, origin_json, origin_stream};

use super::{invalid, require, sha256};

const STREAM: OriginStream = OriginStream::Records {
    kind: MediaKind::AudioControl,
};
const DOCUMENT_FORMAT: &str = "/data/payload/format";

#[derive(Default)]
pub(super) struct Origins {
    ordinal: u64,
    fields: u64,
    emitted: Option<Emitted>,
    eof: bool,
    witnesses: BTreeMap<String, Witness>,
    source_entries: BTreeMap<String, SourceEntry>,
}

struct Emitted {
    digest: String,
    paths: Vec<String>,
    document: Option<DocumentProof>,
    coverage: Coverage,
    ended: bool,
}

struct DocumentProof {
    reference: String,
    encryption: Option<bool>,
}

struct Witness {
    entry: usize,
    size: u64,
    encryption: Encryption,
}

impl Origins {
    pub(super) fn row(&mut self, line: &[u8]) -> io::Result<()> {
        require(!self.eof, "origin rows after EOF")?;
        match read_row(line)? {
            ReadRow::Occurrence {
                scope,
                profile,
                payload_sha256,
            } if scope.stream == STREAM => {
                require(
                    self.emitted.is_none()
                        && self.ordinal == scope.ordinal.get()
                        && profile == OriginProfile::AudioControl,
                    "ATL occurrence identity or ordering",
                )?;
                self.emitted = Some(Emitted {
                    digest: payload_sha256,
                    paths: Vec::new(),
                    document: None,
                    coverage: Coverage::new(),
                    ended: false,
                });
            }
            ReadRow::Field {
                scope,
                field,
                origin,
            } if scope.stream == STREAM => {
                require(
                    scope.ordinal.get() == self.ordinal,
                    "ATL origin field occurrence",
                )?;
                let emitted = self.pending()?;
                if field == DOCUMENT_FORMAT {
                    emitted.document = Some(document_proof(&origin)?);
                }
                emitted
                    .coverage
                    .observe(&origin)
                    .map_err(io::Error::other)?;
                emitted.paths.push(field);
            }
            ReadRow::End { scope, coverage } if scope.stream == STREAM => {
                require(
                    scope.ordinal.get() == self.ordinal,
                    "ATL origin completion scope",
                )?;
                let emitted = self.pending()?;
                let fields = emitted
                    .coverage
                    .finish(&coverage)
                    .map_err(io::Error::other)?;
                emitted.ended = true;
                self.fields = self
                    .fields
                    .checked_add(fields)
                    .ok_or_else(|| invalid("ATL field count overflow"))?;
            }
            ReadRow::Eof { streams } => self.end_stream(&streams)?,
            _ => {}
        }
        Ok(())
    }

    fn end_stream(&mut self, streams: &[OriginStreamSummary]) -> io::Result<()> {
        let mut matches = streams.iter().filter(|summary| summary.stream == STREAM);
        let summary = matches
            .next()
            .ok_or_else(|| invalid("ATL origin EOF summary missing"))?;
        require(
            matches.next().is_none()
                && self.emitted.is_none()
                && summary.occurrences.get() == self.ordinal
                && summary.fields.get() == self.fields,
            "ATL origin EOF multiplicities",
        )?;
        self.eof = true;
        Ok(())
    }

    pub(super) fn record(
        &mut self,
        line: &[u8],
        record: &MediaRecord,
        entry: Option<usize>,
        archive: &Archive,
    ) -> io::Result<()> {
        let emitted = self
            .emitted
            .take()
            .ok_or_else(|| invalid("ATL record without origin occurrence"))?;
        require(
            emitted.ended && emitted.digest == sha256(line),
            "ATL origin payload digest or completion",
        )?;
        let mut paths = emitted.paths.iter();
        origin_json::visit(
            line,
            origin_stream::payload_depth(STREAM, crate::Limits::default().value_depth),
            &mut |path| {
                if paths.next().is_some_and(|expected| expected == path) {
                    Ok(())
                } else {
                    Err(MediaError::Contract(
                        "ATL field address differs from its actual record".into(),
                    ))
                }
            },
        )
        .map_err(io::Error::other)?;
        require(
            paths.next().is_none(),
            "extra field origin outside the ATL payload",
        )?;
        if record.subresource.namespace == wire::DOCUMENT {
            self.document(archive, entry, record.source.size, emitted.document)?;
        }
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or_else(|| invalid("ATL record count overflow"))?;
        Ok(())
    }

    fn document(
        &mut self,
        archive: &Archive,
        entry: Option<usize>,
        size: u64,
        proof: Option<DocumentProof>,
    ) -> io::Result<()> {
        let proof = proof.ok_or_else(|| invalid("ATL document lacks its payload origin"))?;
        let entry = entry.ok_or_else(|| invalid("ATL document lacks physical Entry"))?;
        let physical = archive
            .entries()
            .get(entry)
            .and_then(|physical| archive.observed_entry(physical))
            .ok_or_else(|| invalid("ATL document lacks observed Entry"))?;
        let (encryption, marker) = match physical.stored_encryption() {
            SourceEncryption::Encrypted => (Encryption::Encrypted, Some(true)),
            SourceEncryption::Unencrypted => (Encryption::Unencrypted, Some(false)),
            _ => (Encryption::Unknown, None),
        };
        require(
            proof.encryption == marker,
            "ATL document encryption disagrees with physical Entry",
        )?;
        require(
            self.witnesses
                .insert(
                    proof.reference,
                    Witness {
                        entry,
                        size,
                        encryption,
                    },
                )
                .is_none(),
            "ATL documents share one physical payload witness",
        )
    }

    fn pending(&mut self) -> io::Result<&mut Emitted> {
        self.emitted
            .as_mut()
            .filter(|emitted| !emitted.ended)
            .ok_or_else(|| invalid("ATL origin field outside its occurrence"))
    }

    pub(super) fn source(
        &mut self,
        line: &[u8],
        originals: &BTreeMap<usize, String>,
    ) -> io::Result<()> {
        let source: SourceEntry = serde_json::from_slice(line)?;
        if let Some(entry) = source
            .entry_index
            .as_ref()
            .map(EntryIndex::get)
            .transpose()?
            && originals.contains_key(&entry)
            && matches!(
                source.kind,
                SourceKind::ArchiveEntry | SourceKind::ArchiveEntryProof
            )
        {
            let id = source.id.clone();
            require(
                self.source_entries.insert(id, source).is_none(),
                "duplicate ATL physical source ID",
            )?;
        }
        Ok(())
    }

    pub(super) fn use_row(&mut self, line: &[u8]) -> io::Result<()> {
        let key: UseKey<'_> = serde_json::from_slice(line)?;
        let Some(witness) = self.witnesses.remove(key.id) else {
            return Ok(());
        };
        let used: DocumentUse<'_> = serde_json::from_slice(line)?;
        let source = self
            .source_entries
            .get(used.source)
            .ok_or_else(|| invalid("ATL document witness lacks its inventory source"))?;
        require(
            source
                .entry_index
                .as_ref()
                .map(EntryIndex::get)
                .transpose()?
                == Some(witness.entry)
                && source.ancestors.is_empty()
                && source.stored_encryption == witness.encryption
                && used.effective_encryption == witness.encryption
                && used.scope == Scope::Payload
                && used.role == Role::Payload
                && used.location.kind == "subresource"
                && used.location.namespace == "atl-definition-file"
                && used.location.field == "document"
                && used
                    .location
                    .indices
                    .iter()
                    .map(|value| crate::origin_decimal::parse(value))
                    .eq([Ok(0), Ok(witness.size)]),
            "ATL document witness does not bind its exact original payload and encryption",
        )
    }

    pub(super) fn finish(&self) -> io::Result<()> {
        require(
            self.eof && self.emitted.is_none() && self.witnesses.is_empty(),
            "ATL origins lack EOF or referenced document input rows",
        )
    }
}

fn read_row(line: &[u8]) -> io::Result<ReadRow> {
    let available = line.len();
    let limits = OriginReaderLimits {
        fields: available,
        metadata_bytes: available
            .saturating_mul(1 + size_of::<p4kx_extraction::SourceUseRef>())
            .saturating_add(size_of::<p4kx_extraction::FieldOrigin>()),
        path_bytes: available,
        field_uses: available,
        reference_bytes: available,
    };
    let mut decoder = serde_json::Deserializer::from_slice(line);
    let row = RowSeed(limits).deserialize(&mut decoder)?;
    decoder.end()?;
    Ok(row)
}

fn document_proof(origin: &ReadOrigin) -> io::Result<DocumentProof> {
    let [reference] = origin.local.source_uses_refs() else {
        return Err(invalid(
            "ATL document format needs one physical payload use",
        ));
    };
    require(
        origin.set.is_none() && origin.source_set_complete(),
        "ATL document format lacks complete direct origin",
    )?;
    Ok(DocumentProof {
        reference: reference.as_str().to_owned(),
        encryption: origin.contains_encrypted_source(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceEntry {
    id: String,
    kind: SourceKind,
    entry_index: Option<EntryIndex>,
    stored_encryption: Encryption,
    ancestors: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EntryIndex {
    Numeric(usize),
    Decimal(String),
}

impl EntryIndex {
    fn get(&self) -> io::Result<usize> {
        match self {
            Self::Numeric(index) => Ok(*index),
            Self::Decimal(index) => crate::origin_decimal::parse(index)
                .map_err(invalid)
                .and_then(|value| usize::try_from(value).map_err(io::Error::other)),
        }
    }
}

#[derive(Deserialize)]
struct UseKey<'a> {
    id: &'a str,
}

#[derive(Deserialize)]
struct DocumentUse<'a> {
    source: &'a str,
    scope: Scope,
    role: Role,
    #[serde(rename = "effectiveEncryption")]
    effective_encryption: Encryption,
    #[serde(borrow)]
    location: UseLocation<'a>,
}

#[derive(Deserialize)]
struct UseLocation<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    namespace: &'a str,
    field: &'a str,
    indices: Vec<String>,
}

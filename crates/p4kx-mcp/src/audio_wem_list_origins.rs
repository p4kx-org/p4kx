// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::sound::WemOriginField;
use p4kx_extraction::{
    AudioError, Bank, EmbeddedWem, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, InputScope, SectionId, SourceCoordinateOmission, SourceEvidenceError,
    SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, Wem,
};
use thiserror::Error;

use crate::audio_wem_input::WemOps as _;

use super::{ListedWem, Outcome};

const ROOT: &str = "/structuredContent";
const WORD_BYTES: u64 = size_of::<u32>() as u64;
const WEM_FIELDS: [(&str, WemOriginField); 7] = [
    ("codec", WemOriginField::Codec),
    ("codec_name", WemOriginField::Codec),
    ("channels", WemOriginField::Channels),
    ("sample_rate", WemOriginField::SampleRate),
    ("sample_count", WemOriginField::SampleCount),
    ("duration_seconds", WemOriginField::Duration),
    ("looping", WemOriginField::Loop),
];

/// A field or original bank location could not be represented.
#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Address(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
    #[error("the parsed soundbank header has no source range")]
    MissingHeader,
    #[error("the selected WEM prefix exceeds its source rows")]
    InvalidPrefix,
}

type Result<T> = std::result::Result<T, EvidenceError>;

struct Evidence {
    uses: SourceUses,
    bank: SourceScope,
    complete: bool,
}

pub(super) fn capture(
    bank: &Bank,
    rows: &[ListedWem<'_>],
    cursor: Option<&str>,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let origin = bank.archive_origin();
    let scope = uses.observe(
        origin.map_or(SourceInput::Unknown, SourceInput::Archive),
        InputScope::Payload,
    )?;
    let evidence = Evidence {
        complete: origin.is_some_and(|origin| origin.ancestors().all(|step| step.is_some())),
        uses,
        bank: scope,
    };
    let mut fields = FieldOrigins::new();
    evidence.envelope(bank, cursor, &mut fields)?;
    for (position, row) in rows.iter().enumerate() {
        evidence.row(position, row, &mut fields)?;
    }
    Ok((fields, evidence.uses.freeze()?))
}

impl Evidence {
    fn envelope(&self, bank: &Bank, cursor: Option<&str>, fields: &mut FieldOrigins) -> Result<()> {
        let directory = self.bank.for_scope(InputScope::ArchiveDirectoryMetadata);
        let path = directory.at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        put(fields, ROOT, "archive", self.uses.origin(&[], true))?;
        put(fields, ROOT, "soundbank", self.origin(&[path], true))?;
        let version = bank
            .soundbank()
            .version_byte_range()
            .ok_or(EvidenceError::MissingHeader)?;
        let version = version.start as u64..version.end as u64;
        for (output, original, range) in [
            ("version", "version", version.clone()),
            ("bank_id", "bankId", version.end..version.end + WORD_BYTES),
        ] {
            let word = self.range("bnkBkhd", original, range, SourceUseRole::Payload)?;
            put(fields, ROOT, output, self.origin(&[word], true))?;
        }
        let membership = self.origin(&self.membership(bank)?, true);
        put(fields, ROOT, "total", membership.clone())?;
        put(fields, ROOT, "wems", membership.clone())?;
        if cursor.is_some() {
            put(fields, ROOT, "next_cursor", membership)?;
        }
        Ok(())
    }

    fn membership(&self, bank: &Bank) -> Result<Vec<SourceUse>> {
        let mut decoded_end = 0;
        let mut didx = None;
        let mut data = None;
        for (tag, body) in bank.soundbank().source_section_bodies() {
            decoded_end = body.end;
            match tag {
                SectionId::DIDX => didx = Some(body),
                SectionId::DATA => data = Some(body),
                _ => {}
            }
        }
        let routing = self.site(
            "bnkSectionDirectory",
            "headerScan",
            &[decoded_end],
            SourceUseRole::Selector,
        )?;
        let mut membership = vec![routing];
        if let (Some(didx), Some(data)) = (didx, data) {
            membership.push(self.range("bnkDidx", "triplets", didx, SourceUseRole::Conversion)?);
            membership.push(self.range(
                "bnkData",
                "bodyLength",
                data.start - WORD_BYTES..data.start,
                SourceUseRole::Selector,
            )?);
        }
        Ok(membership)
    }

    fn row(&self, index: usize, row: &ListedWem<'_>, fields: &mut FieldOrigins) -> Result<()> {
        let parent = format!("{ROOT}/wems/{index}");
        let wem = &row.wem;
        let triplet = wem.source().triplet_offset();
        for (output, original, at) in [
            ("id", "mediaId", triplet),
            ("size", "dataLength", triplet + 2 * WORD_BYTES),
        ] {
            let word = self.range(
                "bnkDidx",
                original,
                at..at + WORD_BYTES,
                SourceUseRole::Payload,
            )?;
            put(fields, &parent, output, self.origin(&[word], true))?;
        }
        let placement = wem.source();
        let source = placement.byte_range();
        let selected = self.range(
            "bnkDidx",
            "dataOffset",
            triplet + WORD_BYTES..triplet + 2 * WORD_BYTES,
            SourceUseRole::Selector,
        )?;
        let retained = self.range(
            "bnkDidx",
            "dataLength",
            triplet + 2 * WORD_BYTES..triplet + 3 * WORD_BYTES,
            SourceUseRole::Selector,
        )?;
        let body = placement.data_body_offset();
        let section = self.range(
            "bnkData",
            "bodyLength",
            body - WORD_BYTES..body,
            SourceUseRole::Selector,
        )?;
        let mut read = vec![selected, retained, section];
        if source.end - source.start >= 2 * WORD_BYTES {
            read.push(self.range(
                "bnkWemRiffHeader",
                "signature",
                source.start..source.start + WORD_BYTES,
                SourceUseRole::Conversion,
            )?);
        }
        if wem.declared_len().is_some() {
            read.push(self.range(
                "bnkWemRiffHeader",
                "declaredSize",
                source.start + WORD_BYTES..source.start + 2 * WORD_BYTES,
                SourceUseRole::Conversion,
            )?);
        }
        put(fields, &parent, "prefetch", self.origin(&read, true))?;
        self.outcome(row, &parent, &read, fields)
    }

    fn outcome(
        &self,
        row: &ListedWem<'_>,
        parent: &str,
        read: &[SourceUse],
        fields: &mut FieldOrigins,
    ) -> Result<()> {
        let source = row.wem.source().byte_range();
        let mut disposition = read.to_vec();
        let mut complete = true;
        match &row.outcome {
            Outcome::Prefetch => {}
            Outcome::Parsed(_) => {
                disposition.push(self.row_extent(&row.wem, SourceUseRole::Conversion)?);
            }
            Outcome::Unreadable(AudioError::PluginMedia) => {
                disposition.push(self.range(
                    "bnkWemRiffHeader",
                    "signature",
                    source.start..source.start + WORD_BYTES,
                    SourceUseRole::Conversion,
                )?);
            }
            Outcome::Unreadable(_) => {
                disposition.push(self.row_extent(&row.wem, SourceUseRole::Selector)?);
                disposition.push(self.bank.at_opaque(
                    SourceUseRole::Conversion,
                    SourceCoordinateOmission::Unavailable,
                )?);
                complete = false;
            }
        }
        let status = self.origin(&disposition, complete);
        for name in ["plugin_media", "error"] {
            put(fields, parent, name, status.clone())?;
        }
        let Outcome::Parsed(wem) = &row.outcome else {
            for (name, _) in WEM_FIELDS {
                put(fields, parent, name, status.clone())?;
            }
            return Ok(());
        };
        self.parsed_fields(wem, &source, parent, read, fields)
    }

    fn parsed_fields(
        &self,
        wem: &Wem<&[u8]>,
        source: &Range<u64>,
        parent: &str,
        read: &[SourceUse],
        fields: &mut FieldOrigins,
    ) -> Result<()> {
        let plan = wem.origin_plan();
        for (name, field) in WEM_FIELDS {
            let mut used = read.to_vec();
            let complete = if let Ok(sites) = plan.field(field) {
                for site in sites {
                    let mut indices = vec![source.start, source.end];
                    indices.extend_from_slice(site.indices());
                    used.push(self.bank.at(
                        SourceLocation::Subresource {
                            namespace: format!("bnkWem.{}", site.namespace()),
                            indices,
                            field: site.name().into(),
                        },
                        site.role(),
                    )?);
                }
                true
            } else {
                used.push(self.bank.at_opaque(
                    SourceUseRole::Conversion,
                    SourceCoordinateOmission::Unavailable,
                )?);
                false
            };
            put(fields, parent, name, self.origin(&used, complete))?;
        }
        Ok(())
    }

    fn row_extent(&self, wem: &EmbeddedWem<'_>, role: SourceUseRole) -> Result<SourceUse> {
        self.range(
            "bnkEmbeddedWem",
            "metadataRead",
            wem.source().byte_range(),
            role,
        )
    }

    fn range(
        &self,
        namespace: &str,
        field: &str,
        range: Range<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        self.site(namespace, field, &[range.start, range.end], role)
    }

    fn site(
        &self,
        namespace: &str,
        field: &str,
        indices: &[u64],
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.bank.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: indices.to_vec(),
                field: field.into(),
            },
            role,
        )?)
    }

    fn origin(&self, sites: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(sites, self.complete && complete)
    }
}

fn put(fields: &mut FieldOrigins, parent: &str, field: &str, origin: FieldOrigin) -> Result<()> {
    fields.insert(FieldPath::parse(&format!("{parent}/{field}"))?, origin)?;
    Ok(())
}

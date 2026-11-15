// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::sound::{WemOriginField, WemOriginPlan, WemOriginSite};
use p4kx_extraction::{
    AudioError, EmbeddedWem, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, InputScope, SourceCoordinateOmission, SourceEvidenceError, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use thiserror::Error;

const WORD_BYTES: u64 = size_of::<u32>() as u64;
const PARSED_FIELDS: [(&str, WemOriginField); 7] = [
    ("codec", WemOriginField::Codec),
    ("codec_name", WemOriginField::Codec),
    ("channels", WemOriginField::Channels),
    ("sample_rate", WemOriginField::SampleRate),
    ("sample_count", WemOriginField::SampleCount),
    ("duration_seconds", WemOriginField::Duration),
    ("looping", WemOriginField::Loop),
];

/// A WEM field or original input could not be represented in the response.
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
    #[error("the selected prefix exceeds its source rows")]
    InvalidPrefix,
}

type Result<T> = std::result::Result<T, EvidenceError>;

/// The physical decoded word that supplies one DIDX member field.
#[derive(Clone, Copy)]
pub(super) enum DidxWord {
    Id,
    Offset,
    Length,
}

impl DidxWord {
    pub(super) fn bind(
        self,
        scope: &SourceScope,
        triplet: u64,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        let (position, name) = match self {
            Self::Id => (0, "mediaId"),
            Self::Offset => (1, "dataOffset"),
            Self::Length => (2, "dataLength"),
        };
        let start = triplet + position * WORD_BYTES;
        range_use(scope, "bnkDidx", name, start..start + WORD_BYTES, role)
    }
}

struct EmbeddedPlacement {
    triplet: u64,
    data: u64,
    riff_length: bool,
}

type ParsedSites = [Option<Vec<WemOriginSite>>; PARSED_FIELDS.len()];

enum Outcome {
    Parsed(Box<ParsedSites>),
    Prefetch,
    Plugin,
    Unreadable,
}

/// Original positions consumed by one WEM metadata read.
pub(super) struct PayloadPlan {
    extent: Range<u64>,
    placement: Option<EmbeddedPlacement>,
    outcome: Outcome,
}

impl PayloadPlan {
    pub(super) fn parsed(
        plan: &WemOriginPlan<'_>,
        size: u64,
        embedded: Option<&EmbeddedWem<'_>>,
    ) -> Self {
        let outcome = Outcome::Parsed(Box::new(
            PARSED_FIELDS.map(|(_, field)| plan.field(field).ok()),
        ));
        if let Some(wem) = embedded {
            return Self::embedded(wem, outcome);
        }
        Self {
            extent: 0..size,
            placement: None,
            outcome,
        }
    }

    pub(super) fn embedded_failure(wem: &EmbeddedWem<'_>, error: Option<&AudioError>) -> Self {
        let outcome = match error {
            None => Outcome::Prefetch,
            Some(AudioError::PluginMedia) => Outcome::Plugin,
            Some(_) => Outcome::Unreadable,
        };
        Self::embedded(wem, outcome)
    }

    fn embedded(wem: &EmbeddedWem<'_>, outcome: Outcome) -> Self {
        let source = wem.source();
        Self {
            extent: source.byte_range(),
            placement: Some(EmbeddedPlacement {
                triplet: source.triplet_offset(),
                data: source.data_body_offset(),
                riff_length: wem.declared_len().is_some(),
            }),
            outcome,
        }
    }

    pub(super) fn record(
        &self,
        uses: &SourceUses,
        scope: &SourceScope,
        complete: bool,
        parent: &str,
        fields: &mut FieldOrigins,
    ) -> Result<()> {
        let read = self.prefetch(scope)?;
        put(fields, parent, "prefetch", uses.origin(&read, complete))?;
        let status = self.disposition(scope, &read)?;
        let status = uses.origin(&status, complete);
        for name in ["plugin_media", "error"] {
            put(fields, parent, name, status.clone())?;
        }
        if let Outcome::Parsed(sites) = &self.outcome {
            for ((name, _), plan) in PARSED_FIELDS.iter().zip(sites.iter()) {
                let mut actual = read.clone();
                self.bind_field(scope, plan.as_deref(), &mut actual)?;
                put(fields, parent, name, uses.origin(&actual, complete))?;
            }
        } else {
            for (name, _) in PARSED_FIELDS {
                put(fields, parent, name, status.clone())?;
            }
        }
        Ok(())
    }

    pub(super) fn triplet(&self) -> Option<u64> {
        self.placement.as_ref().map(|site| site.triplet)
    }

    pub(super) fn summary_origin(
        &self,
        uses: &SourceUses,
        scope: &SourceScope,
        complete: bool,
    ) -> Result<FieldOrigin> {
        let read = self.prefetch(scope)?;
        Ok(uses.origin(&self.disposition(scope, &read)?, complete))
    }

    fn prefetch(&self, scope: &SourceScope) -> Result<Vec<SourceUse>> {
        let Some(site) = &self.placement else {
            return Ok(vec![metadata(scope, "path", SourceUseRole::Selector)?]);
        };
        let mut read = vec![
            DidxWord::Offset.bind(scope, site.triplet, SourceUseRole::Selector)?,
            DidxWord::Length.bind(scope, site.triplet, SourceUseRole::Selector)?,
            range_use(
                scope,
                "bnkData",
                "bodyLength",
                site.data - WORD_BYTES..site.data,
                SourceUseRole::Selector,
            )?,
        ];
        if self.extent.end - self.extent.start >= 2 * WORD_BYTES {
            read.push(self.riff_word(scope, 0, "signature")?);
        }
        if site.riff_length {
            read.push(self.riff_word(scope, 1, "declaredSize")?);
        }
        Ok(read)
    }

    fn disposition(&self, scope: &SourceScope, read: &[SourceUse]) -> Result<Vec<SourceUse>> {
        let mut status = read.to_vec();
        match self.outcome {
            Outcome::Prefetch => {}
            Outcome::Parsed(_) => {
                status.push(self.metadata_read(scope, SourceUseRole::Conversion)?);
            }
            Outcome::Plugin => status.push(self.riff_word(scope, 0, "signature")?),
            Outcome::Unreadable => {
                status.push(self.metadata_read(scope, SourceUseRole::Selector)?);
                status.push(incomplete(scope)?);
            }
        }
        Ok(status)
    }

    fn metadata_read(&self, scope: &SourceScope, role: SourceUseRole) -> Result<SourceUse> {
        let namespace = if self.placement.is_some() {
            "bnkEmbeddedWem"
        } else {
            "wemDecodedEntryBytes"
        };
        range_use(scope, namespace, "metadataRead", self.extent.clone(), role)
    }

    fn riff_word(&self, scope: &SourceScope, word: u64, name: &str) -> Result<SourceUse> {
        let start = self.extent.start + word * WORD_BYTES;
        range_use(
            scope,
            "bnkWemRiffHeader",
            name,
            start..start + WORD_BYTES,
            SourceUseRole::Conversion,
        )
    }

    fn bind_field(
        &self,
        scope: &SourceScope,
        plan: Option<&[WemOriginSite]>,
        actual: &mut Vec<SourceUse>,
    ) -> Result<()> {
        let Some(sites) = plan else {
            actual.push(incomplete(scope)?);
            return Ok(());
        };
        for site in sites {
            let used = if self.placement.is_some() {
                let mut indices = vec![self.extent.start, self.extent.end];
                indices.extend_from_slice(site.indices());
                scope.at(
                    SourceLocation::Subresource {
                        namespace: format!("bnkWem.{}", site.namespace()),
                        indices,
                        field: site.name().into(),
                    },
                    site.role(),
                )?
            } else {
                site.bind(scope)?
            };
            actual.push(used);
        }
        Ok(())
    }
}

fn incomplete(scope: &SourceScope) -> Result<SourceUse> {
    Ok(scope.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::Unavailable,
    )?)
}

pub(crate) fn metadata(scope: &SourceScope, field: &str, role: SourceUseRole) -> Result<SourceUse> {
    Ok(scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: field.into(),
        },
        role,
    )?)
}

pub(crate) fn range_use(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices: vec![range.start, range.end],
            field: field.into(),
        },
        role,
    )?)
}

pub(crate) fn put(
    fields: &mut FieldOrigins,
    parent: &str,
    field: &str,
    origin: FieldOrigin,
) -> Result<()> {
    fields.insert(FieldPath::parse(&format!("{parent}/{field}"))?, origin)?;
    Ok(())
}

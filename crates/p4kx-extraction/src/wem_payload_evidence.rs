// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original fields used to describe a decoded or embedded WEM.

use std::ops::Range;

use crate::bank::BankMediaClassSource;
use crate::sound::{WemOriginPlan, WemOriginSite};
use crate::{
    AudioError, EmbeddedWem, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, InputScope, SourceCoordinateOmission, SourceEvidenceError, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use thiserror::Error;

const WORD_BYTES: u64 = size_of::<u32>() as u64;
pub use crate::wem_payload_field::{MetadataField, PayloadField};

/// A WEM field or original input could not be represented in the response.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvidenceError {
    #[error(transparent)]
    /// The original use cannot be bound.
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    /// An address is invalid.
    Address(#[from] FieldPathError),
    #[error(transparent)]
    /// A field was bound more than once.
    Duplicate(#[from] FieldOriginsError),
    #[error("the parsed soundbank header has no source range")]
    /// The parsed header lacks its physical source range.
    MissingHeader,
    #[error("the selected prefix exceeds its source rows")]
    /// The selected rows exceed the observed input.
    InvalidPrefix,
}

type Result<T> = std::result::Result<T, EvidenceError>;

/// The physical decoded word that supplies one DIDX member field.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum DidxWord {
    /// The authored WEM identifier.
    Id,
    /// The authored offset within DATA.
    Offset,
    /// The authored retained length.
    Length,
}

impl DidxWord {
    /// Binds this word within the selected physical triplet.
    ///
    /// # Errors
    /// Returns an error when the original use cannot be recorded.
    pub fn bind(self, scope: &SourceScope, triplet: u64, role: SourceUseRole) -> Result<SourceUse> {
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
    class: BankMediaClassSource,
}

type ParsedSites = [Option<Vec<WemOriginSite>>; MetadataField::ALL.len()];

enum Outcome {
    Parsed(Box<ParsedSites>),
    Prefetch,
    Plugin,
    NonSound,
    Unreadable,
}

/// Original positions consumed by one WEM metadata read.
pub struct PayloadPlan {
    extent: Range<u64>,
    placement: Option<EmbeddedPlacement>,
    outcome: Outcome,
}

impl PayloadPlan {
    /// Captures the fields of an actual WEM parse.
    #[must_use]
    pub fn parsed(plan: &WemOriginPlan<'_>, size: u64, embedded: Option<&EmbeddedWem<'_>>) -> Self {
        let outcome = Outcome::Parsed(Box::new(
            MetadataField::ALL.map(|field| plan.field(field.source_field()).ok()),
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

    /// Captures a prefetched or unreadable embedded item.
    #[must_use]
    pub fn embedded_failure(wem: &EmbeddedWem<'_>, error: Option<&AudioError>) -> Self {
        let outcome = match error {
            None => Outcome::Prefetch,
            Some(AudioError::PluginMedia) => Outcome::Plugin,
            Some(_) => Outcome::Unreadable,
        };
        Self::embedded(wem, outcome)
    }

    /// Captures an embedded item excluded from WEM parsing by its class.
    #[must_use]
    pub fn non_sound(wem: &EmbeddedWem<'_>) -> Self {
        Self::embedded(wem, Outcome::NonSound)
    }

    fn embedded(wem: &EmbeddedWem<'_>, outcome: Outcome) -> Self {
        let source = wem.source();
        Self {
            extent: source.byte_range(),
            placement: Some(EmbeddedPlacement {
                triplet: source.triplet_offset(),
                data: source.data_body_offset(),
                riff_length: wem.declared_len().is_some(),
                class: wem.class_with_source().1,
            }),
            outcome,
        }
    }

    /// Binds fields emitted by a caller at its actual result address.
    ///
    /// # Errors
    /// Returns an error for invalid addresses or unavailable evidence storage.
    pub fn record(
        &self,
        uses: &SourceUses,
        scope: &SourceScope,
        complete: bool,
        parent: &str,
        fields: &mut FieldOrigins,
        selection: &[(&str, PayloadField)],
    ) -> Result<()> {
        let read = self.prefetch(scope)?;
        let status = self.disposition(scope, &read)?;
        let status = uses.origin(&status, complete);
        for &(name, field) in selection {
            let origin = match field {
                PayloadField::Prefetch => uses.origin(&read, complete),
                PayloadField::Status => status.clone(),
                PayloadField::Class => uses.origin(&self.class(scope)?, complete),
                PayloadField::Metadata(index) => {
                    if let Outcome::Parsed(sites) = &self.outcome {
                        let mut actual = read.clone();
                        let plan = sites.get(index.index()).and_then(Option::as_deref);
                        self.bind_field(scope, plan, &mut actual)?;
                        uses.origin(&actual, complete)
                    } else {
                        status.clone()
                    }
                }
            };
            put(fields, parent, name, origin)?;
        }
        Ok(())
    }

    /// Returns the original triplet start of an embedded WEM.
    #[must_use]
    pub fn triplet(&self) -> Option<u64> {
        self.placement.as_ref().map(|site| site.triplet)
    }

    /// Returns the recorded outcome of the WEM read.
    ///
    /// # Errors
    /// Returns an error when the original use cannot be recorded.
    pub fn summary_origin(
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
        let mut read = embedded_extent(scope, site.triplet, site.data)?;
        if self.extent.end - self.extent.start >= 2 * WORD_BYTES {
            read.push(self.riff_word(scope, 0, "signature")?);
        }
        if site.riff_length {
            read.push(self.riff_word(scope, 1, "declaredSize")?);
        }
        Ok(read)
    }

    fn class(&self, scope: &SourceScope) -> Result<Vec<SourceUse>> {
        let site = self
            .placement
            .as_ref()
            .ok_or(EvidenceError::InvalidPrefix)?;
        let mut actual = embedded_extent(scope, site.triplet, site.data)?;
        match site.class {
            BankMediaClassSource::TooShort => {}
            BankMediaClassSource::Signature | BankMediaClassSource::MissingForm => {
                actual.push(self.riff_word(scope, 0, "signature")?);
            }
            BankMediaClassSource::SignatureAndForm => {
                actual.push(self.riff_word(scope, 0, "signature")?);
                actual.push(self.riff_word(scope, 2, "form")?);
            }
            _ => actual.push(incomplete(scope)?),
        }
        Ok(actual)
    }

    fn disposition(&self, scope: &SourceScope, read: &[SourceUse]) -> Result<Vec<SourceUse>> {
        let mut status = read.to_vec();
        match self.outcome {
            Outcome::Prefetch => {}
            Outcome::Parsed(_) => {
                status.push(self.metadata_read(scope, SourceUseRole::Conversion)?);
            }
            Outcome::Plugin => status.push(self.riff_word(scope, 0, "signature")?),
            Outcome::NonSound => status.extend(self.class(scope)?),
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
                site.bind_embedded(scope, self.extent.clone())?
            } else {
                site.bind(scope)?
            };
            actual.push(used);
        }
        Ok(())
    }
}

pub(crate) fn embedded_extent(
    scope: &SourceScope,
    triplet: u64,
    data: u64,
) -> Result<Vec<SourceUse>> {
    Ok(vec![
        DidxWord::Offset.bind(scope, triplet, SourceUseRole::Selector)?,
        DidxWord::Length.bind(scope, triplet, SourceUseRole::Selector)?,
        range_use(
            scope,
            "bnkData",
            "bodyLength",
            data - WORD_BYTES..data,
            SourceUseRole::Selector,
        )?,
    ])
}

fn incomplete(scope: &SourceScope) -> Result<SourceUse> {
    Ok(scope.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::Unavailable,
    )?)
}

/// Binds an original Archive directory metadata field.
///
/// # Errors
/// Returns an error when the original use cannot be recorded.
pub fn metadata(scope: &SourceScope, field: &str, role: SourceUseRole) -> Result<SourceUse> {
    Ok(scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: field.into(),
        },
        role,
    )?)
}

/// Binds an original half-open range in a decoded sound source.
///
/// # Errors
/// Returns an error when the original use cannot be recorded.
pub fn range_use(
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

/// Attaches an origin to an address in an output object.
///
/// # Errors
/// Returns an error for invalid or duplicate addresses.
pub fn put(
    fields: &mut FieldOrigins,
    parent: &str,
    field: &str,
    origin: FieldOrigin,
) -> Result<()> {
    fields.insert(FieldPath::parse(&format!("{parent}/{field}"))?, origin)?;
    Ok(())
}

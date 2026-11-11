// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use crate::projection_schema;
use crate::source_details::SourceCoordinateOmission;
use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};
use crate::{DecimalU64, Language};

/// A document-local source-use identifier, independent of addresses or pointer values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct SourceUseRef(pub(crate) String);

impl SourceUseRef {
    /// Returns the stable document-local reference text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether an observed input actually contributes to the emitted data value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum SourceUseRole {
    /// Retained payload bytes or facts.
    Payload,
    /// An authored selector used to select this result.
    Selector,
    /// A used input to the value's conversion.
    Conversion,
    /// A planned or failed input without a retained contribution.
    Attempted,
    /// An untouched reference target.
    Reference,
}

impl SourceUseRole {
    pub(crate) const fn contributes(self) -> bool {
        matches!(self, Self::Payload | Self::Selector | Self::Conversion)
    }

    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Payload => "payload",
            Self::Selector => "selector",
            Self::Conversion => "conversion",
            Self::Attempted => "attempted",
            Self::Reference => "reference",
        }
    }
}

/// The source byte domain interpreted by one use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub enum InputScope {
    /// The Entry or standalone file's payload, including every containing Entry.
    Payload,
    /// Directory facts, with only containing Entries affecting their origin.
    ArchiveDirectoryMetadata,
    /// Observed standalone-file metadata without original P4K lineage proof.
    FileMetadata,
}

impl InputScope {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Payload => "payload",
            Self::ArchiveDirectoryMetadata => "metadata",
            Self::FileMetadata => "fileMetadata",
        }
    }
}

/// A retained source coordinate or an explicit omission inside an observed physical input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SourceLocation {
    /// An actual input use with explicitly omitted optional coordinates.
    Opaque {
        /// Why coordinate details were not retained.
        reason: SourceCoordinateOmission,
    },
    /// One field of the physical DataForge Record index.
    DataForge {
        /// Physical Record position, independent of nominal GUID aliases.
        record_index: u32,
        /// Exact owned-field path, including stored list positions.
        field: String,
    },
    /// A localization lookup addressed by its retained authored selector.
    LocalizationLookup {
        /// The language Entry used by the lookup.
        #[serde(serialize_with = "language_code")]
        #[schemars(with = "String")]
        language: Language,
        /// The source use holding the actual lookup key.
        selector: SourceUseRef,
    },
    /// One exact logical field inside an already observed structured source subresource.
    Subresource {
        /// Format-owned namespace for the original source object.
        namespace: String,
        /// Original source ordinals, including repeated declarations and unknown slots.
        #[serde(serialize_with = "subresource_indices")]
        #[schemars(with = "Vec<DecimalU64>")]
        indices: Vec<u64>,
        /// Original source field or token-range label within that object.
        field: String,
    },
    /// A directory or standalone input metadata field.
    Metadata {
        /// Exact metadata field, including stored occurrence positions.
        field: String,
    },
}

impl SourceLocation {
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Opaque { reason } => format!("o:{}", reason.key()),
            Self::DataForge {
                record_index,
                field,
            } => format!("d{record_index}:{}:{field}", field.len()),
            Self::LocalizationLookup { language, selector } => {
                let code = language.code();
                format!("q{}:{code}{}:{}", code.len(), selector.0.len(), selector.0)
            }
            Self::Subresource {
                namespace,
                indices,
                field,
            } => {
                use std::fmt::Write as _;
                let mut key = format!("s{}:{namespace}{}:", namespace.len(), indices.len());
                for index in indices {
                    let _written = write!(&mut key, "{index}.");
                }
                let _written = write!(&mut key, "{}:{field}", field.len());
                key
            }
            Self::Metadata { field } => format!("m{}:{field}", field.len()),
        }
    }
}

fn subresource_indices<S: Serializer>(indices: &[u64], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(indices.iter().copied().map(DecimalU64::new))
}
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde field serializers receive references"
)]
fn language_code<S: Serializer>(language: &Language, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(language.code())
}

/// A registered observed use and its captured encryption evidence.
#[derive(Debug, Clone)]
pub struct SourceUse {
    pub(crate) description: Arc<SourceUseDescription>,
    pub(crate) identity: Arc<()>,
}

impl SourceUse {
    /// Returns this use's stable document-local reference.
    #[must_use]
    pub fn reference(&self) -> &SourceUseRef {
        &self.description.id
    }

    /// Returns whether this observation supplied payload, selection or conversion input.
    #[must_use]
    pub fn role(&self) -> SourceUseRole {
        self.description.role
    }

    /// Borrows the original registered observation without restoring authority from wire metadata.
    #[must_use]
    pub fn description(&self) -> &SourceUseDescription {
        &self.description
    }

    /// Returns the captured effective encryption in this original observation's byte domain.
    #[must_use]
    pub fn encryption(&self) -> SourceEncryptionState {
        self.description.effective_encryption()
    }
}

/// One source-use inventory row, kept separately from the field's actual contribution set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = projection_schema::require_properties)]
pub struct SourceUseDescription {
    pub(crate) id: SourceUseRef,
    pub(crate) source: PhysicalSourceRef,
    pub(crate) scope: InputScope,
    pub(crate) role: SourceUseRole,
    pub(crate) location: SourceLocation,
    pub(crate) effective_encryption: SourceEncryptionState,
}

impl SourceUseDescription {
    /// Returns this observation's document-local reference.
    #[must_use]
    pub const fn id(&self) -> &SourceUseRef {
        &self.id
    }

    /// Returns the physical source this observation addresses.
    #[must_use]
    pub const fn source(&self) -> &PhysicalSourceRef {
        &self.source
    }

    /// Returns the source byte domain used by this observation.
    #[must_use]
    pub const fn scope(&self) -> InputScope {
        self.scope
    }

    /// Returns whether this observation contributed or only attempted/referenced an input.
    #[must_use]
    pub const fn role(&self) -> SourceUseRole {
        self.role
    }

    /// Returns the retained source coordinate or its explicit omission.
    #[must_use]
    pub const fn location(&self) -> &SourceLocation {
        &self.location
    }

    /// Returns the effective encryption captured for this observation's original byte domain.
    #[must_use]
    pub const fn effective_encryption(&self) -> SourceEncryptionState {
        self.effective_encryption
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceLocation, SourceUseRef};
    use crate::Language;
    use serde_json::json;

    #[test]
    fn localization_schema_requires_an_opaque_selector_instead_of_a_raw_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let location = SourceLocation::LocalizationLookup {
            language: Language::English,
            selector: SourceUseRef("a0:d0:11:description:selector".into()),
        };
        let value = serde_json::to_value(location)?;
        assert_eq!(value["type"], "localizationLookup", "lookup variant");
        assert!(
            value.get("key").is_none(),
            "the serialized location holds no authored key"
        );
        let schema = serde_json::to_value(schemars::schema_for!(SourceLocation))?;
        let validator = jsonschema::validator_for(&schema)?;
        assert!(
            validator.is_valid(&value),
            "the real selector location validates"
        );
        assert!(
            !validator.is_valid(&json!({
                "type": "localization", "language": "en", "key": "@fixture_description"
            })),
            "the schema rejects the removed raw-key variant"
        );
        assert!(
            !validator.is_valid(&json!({
                "type": "localizationLookup", "language": "en", "key": "@fixture_description"
            })),
            "a raw key cannot replace the required source selector"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "source_use_runtime_tests.rs"]
mod runtime_tests;

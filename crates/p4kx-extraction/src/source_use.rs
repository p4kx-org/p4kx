// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "source_xml_location.rs"]
mod xml_location;

#[path = "source_location.rs"]
mod location;
#[path = "source_location_storage.rs"]
pub(crate) mod storage;

pub use location::SourceLocation;
pub(crate) use location::{LocationView, Subresource};
use storage::RetainedLocation;

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use crate::projection_schema;
use crate::source_inventory::{PhysicalSourceRef, SourceEncryptionState};

/// A document-local source-use identifier, independent of addresses or pointer values.
#[derive(Clone, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct SourceUseRef(#[schemars(with = "String")] ReferenceText);

#[derive(Clone)]
enum ReferenceText {
    Registered(Arc<str>),
    Decoded(Box<str>),
}

impl ReferenceText {
    fn as_str(&self) -> &str {
        match self {
            Self::Registered(text) => text,
            Self::Decoded(text) => text,
        }
    }
}

impl Serialize for ReferenceText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl SourceUseRef {
    pub(crate) fn registered(text: String) -> Self {
        Self::from_registered(text.into())
    }

    pub(crate) const fn from_registered(text: Arc<str>) -> Self {
        Self(ReferenceText::Registered(text))
    }

    pub(crate) const fn registered_text(&self) -> Option<&Arc<str>> {
        match &self.0 {
            ReferenceText::Registered(text) => Some(text),
            ReferenceText::Decoded(_) => None,
        }
    }

    pub(crate) const fn decoded(text: Box<str>) -> Self {
        Self(ReferenceText::Decoded(text))
    }

    /// Returns the stable document-local reference text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// A borrowed origin ID from a use description or a registration key.
#[derive(Clone, Copy)]
pub(crate) enum OriginUseId<'a> {
    Description(&'a SourceUseRef),
    Registered(&'a Arc<str>),
}

impl OriginUseId<'_> {
    pub(crate) fn retain(self) -> SourceUseRef {
        match self {
            Self::Description(reference) => reference.clone(),
            Self::Registered(text) => SourceUseRef::from_registered(Arc::clone(text)),
        }
    }
}

impl fmt::Debug for SourceUseRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SourceUseRef")
            .field(&self.as_str())
            .finish()
    }
}

impl PartialEq for SourceUseRef {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SourceUseRef {}

impl Hash for SourceUseRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for SourceUseRef {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceUseRef {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
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
    #[schemars(with = "SourceLocation")]
    pub(crate) location: RetainedLocation,
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
    ///
    /// A compact coordinate materializes its owned view on the first call.
    #[must_use]
    pub fn location(&self) -> &SourceLocation {
        self.location.original()
    }

    pub(crate) const fn is_opaque(&self) -> bool {
        self.location.is_opaque()
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
            selector: SourceUseRef::registered("a0:d0:11:description:selector".into()),
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

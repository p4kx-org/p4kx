// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Build, SourceInventory, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ProfileElement;
use super::{GENERAL, VR};
use crate::catalog_read::original::PhysicalEntryIdentity;
use crate::schema_fields::every_field_required;

/// Selects an authored profile by its archive document role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum ProfileKind {
    /// Default general client attributes.
    General,
    /// Default virtual-reality client attributes.
    VirtualReality,
}

impl ProfileKind {
    pub(super) const fn path(self) -> &'static str {
        match self {
            Self::General => GENERAL,
            Self::VirtualReality => VR,
        }
    }
}

/// The observed outcome of selecting and reading a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum ProfileState {
    /// The physical profile was decoded.
    Read,
    /// The complete Archive directory has no matching Entry.
    Missing,
    /// The request excluded the payload.
    Filtered,
    /// Directory evidence was insufficient to confirm the source.
    Unavailable,
    /// Several physical directory occurrences match the requested path.
    Ambiguous,
    /// A selected physical payload could not be read or verified.
    Unreadable,
    /// The payload could not be decoded as lossless XML.
    Malformed,
    /// The decoded XML uses an unsupported root element.
    Unsupported,
}

/// One requested profile with its physical candidates and decoded contents.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ProfileDocument {
    pub(super) kind: ProfileKind,
    pub(super) requested_path: &'static str,
    pub(super) status: SourcedValue<ProfileState>,
    pub(super) candidates: Vec<PhysicalEntryIdentity>,
    pub(super) elements: Vec<ProfileElement>,
}

impl ProfileDocument {
    /// Returns the role used to select this file.
    #[must_use]
    pub const fn kind(&self) -> ProfileKind {
        self.kind
    }

    /// Returns the canonical path used to query the Archive directory.
    #[must_use]
    pub const fn requested_path(&self) -> &'static str {
        self.requested_path
    }

    /// Borrows the selection and read outcome with its source evidence.
    #[must_use]
    pub const fn status(&self) -> &SourcedValue<ProfileState> {
        &self.status
    }

    /// Lists all matching physical Entries in directory order.
    #[must_use]
    pub fn candidates(&self) -> &[PhysicalEntryIdentity] {
        &self.candidates
    }

    /// Lists decoded elements in document order.
    #[must_use]
    pub fn elements(&self) -> &[ProfileElement] {
        &self.elements
    }
}

/// The authored General and VR client profile documents.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ClientProfileSettings {
    pub(super) documents: [ProfileDocument; 2],
}

impl ClientProfileSettings {
    /// Lists the general and virtual-reality profiles in canonical order.
    #[must_use]
    pub const fn documents(&self) -> &[ProfileDocument] {
        &self.documents
    }

    /// Borrows the selection outcome for a profile role.
    #[must_use]
    pub const fn document(&self, kind: ProfileKind) -> &ProfileDocument {
        let [general, virtual_reality] = &self.documents;
        match kind {
            ProfileKind::General => general,
            ProfileKind::VirtualReality => virtual_reality,
        }
    }

    /// Returns whether a document could not be selected or decoded safely.
    #[must_use]
    pub fn has_failed_inputs(&self) -> bool {
        self.documents.iter().any(|document| {
            !matches!(
                document.status.value(),
                Some(ProfileState::Read | ProfileState::Missing | ProfileState::Filtered)
            )
        })
    }

    /// Returns the number of documents successfully decoded.
    #[must_use]
    pub fn decoded_documents(&self) -> usize {
        self.documents
            .iter()
            .filter(|document| document.status.value() == Some(&ProfileState::Read))
            .count()
    }
}

/// A standalone client profile result with the requested source detail.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ClientProfileSettingsReport {
    pub(super) settings: ClientProfileSettings,
    pub(super) provenance: SourceInventory,
    #[serde(skip)]
    #[schemars(skip)]
    pub(super) build: Option<Build>,
}

impl ClientProfileSettingsReport {
    /// Borrows the Build reported by the checked Archive, if available.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns the observed profile documents.
    #[must_use]
    pub const fn settings(&self) -> &ClientProfileSettings {
        &self.settings
    }

    /// Returns the source inventory, which is empty in marker mode.
    #[must_use]
    pub const fn provenance(&self) -> &SourceInventory {
        &self.provenance
    }
}

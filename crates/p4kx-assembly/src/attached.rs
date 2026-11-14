// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    ArchiveMaterialRead, EntryOrigin, MaterialArchiveOrigins, ModelArchiveSource,
};
use p4kx_model::{MaterialStatus, Model, Transform};

use crate::placement::BoneSelection;
use crate::{Assembly, CdfAttachmentSite};

/// A rigid model a part carries at a bone of its skeleton: a bone attachment of the character
/// definition the part's geometry is, e.g. the housing of a thruster.
#[derive(Debug, Clone)]
pub struct AttachedModel {
    pub(crate) name: String,
    pub(crate) bone: String,
    pub(crate) local: Transform,
    pub(crate) found: bool,
    pub(crate) bone_selection: Option<BoneSelection>,
    pub(crate) cdf_sources: Option<Arc<[CdfAttachmentSite]>>,
    pub(crate) offset_used: bool,
    pub(crate) entry: String,
    pub(crate) model: Arc<Model>,
    pub(crate) archive_sources: Arc<[ModelArchiveSource]>,
    pub(crate) definition_origin: Option<EntryOrigin>,
    pub(crate) materials: Arc<ArchiveMaterialRead>,
}

impl AttachedModel {
    /// Returns the attachment's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the bone of the part's skeleton the model is bound to.
    #[must_use]
    pub fn bone(&self) -> &str {
        &self.bone
    }

    /// Returns whether the part's skeleton has the bone; when not, the model sits at the part's
    /// origin, offset by the attachment's own position and rotation.
    #[must_use]
    pub const fn is_bone_found(&self) -> bool {
        self.found
    }

    /// Returns the transform from the model's space to the part's, in `CryEngine` coordinates, Z
    /// up and in metres: the bone in bind pose, offset by the attachment.
    #[must_use]
    pub const fn local(&self) -> &Transform {
        &self.local
    }

    /// Returns the path of the model's file.
    #[must_use]
    pub fn geometry(&self) -> &str {
        &self.entry
    }

    /// Returns the model.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Returns the original Archive reads of the model retained by this attachment.
    #[must_use]
    pub fn model_archive_sources(&self) -> &[ModelArchiveSource] {
        &self.archive_sources
    }

    /// Returns the actual character definition that selected this attachment, if observed.
    #[must_use]
    pub const fn model_definition_origin(&self) -> Option<&EntryOrigin> {
        self.definition_origin.as_ref()
    }

    /// Returns original CDF attributes retained for this attachment.
    #[must_use]
    pub fn cdf_sources(&self) -> &[CdfAttachmentSite] {
        self.cdf_sources.as_deref().unwrap_or_default()
    }

    /// Returns whether the authored relative transform contributed to placement.
    #[must_use]
    pub const fn offset_used(&self) -> bool {
        self.offset_used
    }

    /// Returns the original observations of the material library used by this attachment.
    #[must_use]
    pub fn material_archive_origins(&self) -> &MaterialArchiveOrigins {
        &self.materials.origins
    }

    /// Returns the material library the model is drawn with.
    #[must_use]
    pub fn materials(&self) -> &MaterialStatus {
        &self.materials.status
    }
}

impl Assembly {
    /// Returns the bone attachments of all parts, each with the index of its part.
    pub fn attached_models(&self) -> impl Iterator<Item = (usize, &AttachedModel)> {
        self.indexed_attachments()
            .map(|(part, _, attachment)| (part, attachment))
    }

    /// Returns each bone attachment with its owning part and local attachment ordinal.
    pub fn indexed_attachments(&self) -> impl Iterator<Item = (usize, usize, &AttachedModel)> {
        self.parts.iter().enumerate().flat_map(|(part, owner)| {
            owner
                .attachments()
                .iter()
                .enumerate()
                .map(move |(ordinal, attachment)| (part, ordinal, attachment))
        })
    }
}

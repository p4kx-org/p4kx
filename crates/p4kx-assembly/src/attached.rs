// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    ArchiveMaterialRead, EntryOrigin, MaterialArchiveOrigins, ModelArchiveSource, SourceLocation,
    SourceUseRole,
};
use p4kx_model::character_definition::AttachmentPoseSelection;
use p4kx_model::{MaterialStatus, Model, Transform};

use crate::placement::BoneSelection;
use crate::{Assembly, CdfAttachmentField, CdfAttachmentSite};

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
    pub(crate) placed_pose: Option<AttachmentPoseSelection>,
    pub(crate) entry: String,
    pub(crate) model: Arc<Model>,
    pub(crate) archive_sources: Arc<[ModelArchiveSource]>,
    pub(crate) definition_origin: Option<EntryOrigin>,
    pub(crate) materials: Arc<ArchiveMaterialRead>,
    pub(crate) material_selection: MaterialSelection,
}

#[derive(Debug, Clone)]
pub(crate) enum MaterialSelection {
    Attachment,
    Entity(Option<SourceLocation>),
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

    /// Returns whether the skeleton contains the selected bone.
    #[must_use]
    pub const fn is_bone_found(&self) -> bool {
        self.found
    }

    /// Returns the transform from the model's space to the part's, in `CryEngine` coordinates, Z
    /// up and in metres.
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

    /// Returns whether the selected pose supplied a valid placement.
    #[must_use]
    pub const fn offset_used(&self) -> bool {
        self.placed_pose.is_some()
    }

    /// Returns whether placement uses the bone bind pose or its identity fallback.
    #[must_use]
    pub const fn placement_uses_bone_pose(&self) -> bool {
        match self.placed_pose {
            Some(pose) => pose.uses_bone(),
            None => true,
        }
    }

    /// Returns the original CDF operands and their roles in the emitted placement.
    pub fn placement_cdf_sources(
        &self,
    ) -> impl Iterator<Item = (&CdfAttachmentSite, SourceUseRole)> {
        self.cdf_sources().iter().filter_map(|source| {
            let field = source.field();
            let role = if let Some(pose_field) = field.pose_field() {
                let pose = self.placed_pose?;
                if pose.uses(pose_field) {
                    SourceUseRole::Conversion
                } else if pose.selects_default(pose_field) {
                    SourceUseRole::Selector
                } else {
                    return None;
                }
            } else if matches!(
                field,
                CdfAttachmentField::Type
                    | CdfAttachmentField::Binding
                    | CdfAttachmentField::BoneName
            ) {
                SourceUseRole::Selector
            } else {
                return None;
            };
            Some((source, role))
        })
    }

    /// Returns the Entity material selector when it overrides this attachment's library.
    #[must_use]
    pub const fn material_selector(&self) -> Option<&SourceLocation> {
        match &self.material_selection {
            MaterialSelection::Attachment => None,
            MaterialSelection::Entity(location) => location.as_ref(),
        }
    }

    /// Returns whether the attachment's own material declaration is eligible to select its library.
    #[must_use]
    pub const fn uses_attachment_material(&self) -> bool {
        matches!(self.material_selection, MaterialSelection::Attachment)
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

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use p4kx_extraction::{DataForgeOrigin, EntryOrigin, ModelArchiveSource, SourceLocation};
use p4kx_model::{Model, ModelFile};

use super::ArmourPart;
use crate::PortPlacementSource;
use crate::placement::BoneSelection;

/// Original inputs retained when a rigid worn model is mounted.
#[derive(Debug, Clone)]
pub struct WornMountSource {
    port: PortPlacementSource,
    forge_origin: Option<Arc<DataForgeOrigin>>,
    holder: Option<MountModelSource>,
    skeleton: Option<MountModelSource>,
    item_locator: Option<MountModelSource>,
}

impl WornMountSource {
    pub(super) const fn new(
        port: PortPlacementSource,
        forge_origin: Option<Arc<DataForgeOrigin>>,
        holder: Option<MountModelSource>,
        skeleton: Option<MountModelSource>,
        item_locator: Option<MountModelSource>,
    ) -> Self {
        Self {
            port,
            forge_origin,
            holder,
            skeleton,
            item_locator,
        }
    }

    /// Returns the original port fields used for the placement.
    #[must_use]
    pub const fn port(&self) -> &PortPlacementSource {
        &self.port
    }

    pub(super) fn forge_origin(&self) -> Option<&DataForgeOrigin> {
        self.forge_origin.as_deref()
    }

    /// Returns the model bone that supplied the attachment frame.
    #[must_use]
    pub const fn holder_bone(&self) -> Option<&MountModelSource> {
        self.holder.as_ref()
    }

    /// Returns the body bone whose inverse pose was used.
    #[must_use]
    pub const fn skeleton_bone(&self) -> Option<&MountModelSource> {
        self.skeleton.as_ref()
    }

    /// Returns the item node whose inverse transform was used.
    #[must_use]
    pub const fn item_locator(&self) -> Option<&MountModelSource> {
        self.item_locator.as_ref()
    }
}

/// Physical fields of one model's selected bone or locator.
#[derive(Debug, Clone)]
pub struct MountModelSource {
    origin: EntryOrigin,
    chunk: u64,
    ordinal: u64,
    name: Option<Range<u64>>,
    transform: Range<u64>,
    is_bone: bool,
}

impl MountModelSource {
    /// Returns the physical Entry supplying the selected fields.
    #[must_use]
    pub const fn origin(&self) -> &EntryOrigin {
        &self.origin
    }

    /// Returns the original name bytes.
    #[must_use]
    pub fn name(&self) -> Option<SourceLocation> {
        Some(self.location("name", self.name.clone()?))
    }

    /// Returns the selected bone's relative pose or the locator's local transform.
    #[must_use]
    pub fn transform(&self) -> SourceLocation {
        let field = if self.is_bone {
            "relativePose"
        } else {
            "localTransform"
        };
        self.location(field, self.transform.clone())
    }

    fn location(&self, field: &str, range: Range<u64>) -> SourceLocation {
        let namespace = if self.is_bone {
            "modelBonePlacementBytes"
        } else {
            "modelNodePlacementBytes"
        };
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices: vec![self.chunk, self.ordinal, range.start, range.end],
            field: field.into(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PlacementModel<'a> {
    pub(super) model: &'a Model,
    sources: &'a [ModelArchiveSource],
}

impl<'a> PlacementModel<'a> {
    pub(super) const fn new(model: &'a Model, sources: &'a [ModelArchiveSource]) -> Self {
        Self { model, sources }
    }

    pub(super) fn part(part: &'a ArmourPart) -> Self {
        Self::new(&part.model, &part.archive_sources)
    }

    pub(super) fn bone_source(self, selection: BoneSelection) -> Option<MountModelSource> {
        let source = self.source(selection.file)?;
        let fields = source.bone_sources()?;
        let bone = fields.bone(selection.ordinal)?;
        Some(MountModelSource {
            origin: source.origin()?.clone(),
            chunk: u64::try_from(fields.chunk()).ok()?,
            ordinal: u64::try_from(selection.ordinal).ok()?,
            name: Some(bone.name()),
            transform: bone.relative_pose(),
            is_bone: true,
        })
    }

    pub(super) fn node_source(self, ordinal: usize) -> Option<MountModelSource> {
        let source = self.source(ModelFile::Model)?;
        let node = source.node_sources()?.node(ordinal)?;
        Some(MountModelSource {
            origin: source.origin()?.clone(),
            chunk: u64::try_from(node.chunk()).ok()?,
            ordinal: u64::try_from(ordinal).ok()?,
            name: node.name(),
            transform: node.matrix(),
            is_bone: false,
        })
    }

    fn source(self, file: ModelFile) -> Option<&'a ModelArchiveSource> {
        self.sources.iter().find(|source| source.file() == file)
    }
}

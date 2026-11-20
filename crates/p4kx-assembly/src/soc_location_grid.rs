// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveWatch, CancelToken, EntryOrigin, InputScope, PathIndex, SourceEvidenceError,
    SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
    directory_path_uses,
};
use p4kx_model::{
    SocContainerDescriptor, SocContainerDescriptorError, SocLocationPhysics, SocPhysicsDeclaration,
    SocPhysicsField, SocSourceValue,
};
use thiserror::Error;

use super::{CaptureCheck, soc_geometry_capture};
use crate::{SocCaptureError, SocGeometryTarget, SocMemberError, SocMemberSelection};

/// Selection of a location grid's physical descriptor.
#[derive(Debug)]
#[non_exhaustive]
pub enum SocLocationGridSelection<'source> {
    /// No descriptor member matches the container.
    Missing,
    /// Several physical descriptors match in the same selection tier.
    Ambiguous(&'source [EntryOrigin]),
    /// The selected original descriptor bytes could not be read.
    Unreadable {
        /// Selected descriptor Entry.
        origin: &'source EntryOrigin,
        /// Original failure status.
        reason: &'source SocMemberError,
    },
    /// The descriptor XML or its root is invalid.
    Invalid {
        /// Selected descriptor Entry.
        origin: &'source EntryOrigin,
        /// Original decoding or root failure.
        reason: SocContainerDescriptorError,
    },
    /// The descriptor decoder reports a lossy read.
    Lossy {
        /// Selected descriptor Entry.
        origin: &'source EntryOrigin,
        /// Original XML retained for inspection.
        descriptor: SocContainerDescriptor<'source>,
    },
    /// The original descriptor was read and can prove presence or absence.
    Read(SocLocationGridSource<'source>),
}

impl<'source> SocLocationGridSelection<'source> {
    /// Reads the typed location-grid source from an already selected SOC descriptor.
    #[must_use]
    pub fn read(selection: &'source SocMemberSelection) -> Self {
        let member = match selection {
            SocMemberSelection::Missing => return Self::Missing,
            SocMemberSelection::Ambiguous(entries) => return Self::Ambiguous(entries),
            SocMemberSelection::Selected(member) => member,
        };
        let origin = member.origin();
        let bytes = match member.payload() {
            Ok(bytes) => bytes,
            Err(reason) => return Self::Unreadable { origin, reason },
        };
        match SocContainerDescriptor::parse(bytes) {
            Ok(descriptor) if descriptor.document().is_lossy() => {
                Self::Lossy { origin, descriptor }
            }
            Ok(descriptor) => Self::Read(SocLocationGridSource { origin, descriptor }),
            Err(reason) => Self::Invalid { origin, reason },
        }
    }
}

/// One verified descriptor from which grid fields and voxel references are read.
#[derive(Debug)]
pub struct SocLocationGridSource<'source> {
    origin: &'source EntryOrigin,
    descriptor: SocContainerDescriptor<'source>,
}

impl SocLocationGridSource<'_> {
    /// Returns the physical descriptor with its full containing Entry ancestry.
    #[must_use]
    pub const fn origin(&self) -> &EntryOrigin {
        self.origin
    }

    /// Returns the original descriptor XML for controls and unknown fields.
    #[must_use]
    pub const fn descriptor(&self) -> &SocContainerDescriptor<'_> {
        &self.descriptor
    }

    /// Returns the authored radius and positioned location physics declarations.
    #[must_use]
    pub fn physics(&self) -> SocLocationPhysics<'_> {
        self.descriptor.location_physics()
    }

    /// Binds a typed control to its original descriptor and exact XML sites.
    ///
    /// # Errors
    /// Returns an error for foreign fields, unknown position formats or failed source binding.
    pub fn sourced<T>(
        &self,
        field: SocPhysicsField<'_, T>,
        sources: &SourceUses,
    ) -> Result<SourcedValue<T>, SocLocationGridError> {
        let uses = self.field_uses(&field, sources, SourceUseRole::Payload)?;
        let value = match field.into_value() {
            SocSourceValue::Absent => SourceValue::Absent,
            SocSourceValue::Present(value) => SourceValue::Present(value),
            _ => SourceValue::Invalid,
        };
        Ok(sources.value(value, &uses, true))
    }

    /// Resolves an original voxel path within explicit caller-provided lookup tiers.
    ///
    /// The caller supplies the logical ancestor order when the descriptor is shared.
    ///
    /// # Errors
    /// Returns an error for foreign selectors, an unbound root, cancellation or directory failure.
    pub fn resolve_voxel(
        &self,
        declaration: SocPhysicsDeclaration<'_>,
        layers: &[PathIndex<'_>],
        cancel: &CancelToken,
    ) -> Result<SocPhysicsVoxelTarget, SocLocationGridError> {
        self.require_owner(declaration.element())?;
        let field = declaration
            .filename()
            .ok_or(SocLocationGridError::NotVoxel)?;
        let first = layers.first().ok_or(SocLocationGridError::Owner)?;
        if !first.archive().owns_origin(self.origin)
            || layers.iter().any(|index| {
                !index
                    .archive()
                    .input_origin()
                    .is_same_input(self.origin.input_origin())
            })
        {
            return Err(SocLocationGridError::Owner);
        }
        let watch = ArchiveWatch::new(first.archive());
        let check = CaptureCheck {
            watch: &watch,
            cancel,
        };
        check.poll()?;
        let target = match field.into_value() {
            SocSourceValue::Absent => SocPhysicsVoxelTarget::Absent,
            SocSourceValue::Present(path) => SocPhysicsVoxelTarget::Path(
                soc_geometry_capture::resolve_path(path, layers, check)?,
            ),
            _ => SocPhysicsVoxelTarget::Path(SocGeometryTarget::InvalidSelector),
        };
        watch.check().map_err(SocCaptureError::from)?;
        Ok(target)
    }

    /// Resolves one voxel and binds its original selector and candidate directory path uses.
    ///
    /// # Errors
    /// Returns the corresponding resolution or source registry error.
    pub fn sourced_voxel(
        &self,
        declaration: SocPhysicsDeclaration<'_>,
        layers: &[PathIndex<'_>],
        cancel: &CancelToken,
        sources: &SourceUses,
    ) -> Result<SourcedValue<SocPhysicsVoxelTarget>, SocLocationGridError> {
        let target = self.resolve_voxel(declaration, layers, cancel)?;
        let field = declaration
            .filename()
            .ok_or(SocLocationGridError::NotVoxel)?;
        let mut uses = self.field_uses(&field, sources, SourceUseRole::Selector)?;
        let candidates = match &target {
            SocPhysicsVoxelTarget::Path(SocGeometryTarget::Matched { entry, .. }) => {
                std::slice::from_ref(entry)
            }
            SocPhysicsVoxelTarget::Path(SocGeometryTarget::Ambiguous { entries, .. }) => entries,
            _ => &[],
        };
        for candidate in candidates {
            let scope = sources.observe(
                SourceInput::Archive(candidate),
                InputScope::ArchiveDirectoryMetadata,
            )?;
            let (path, _) = directory_path_uses(&scope, candidate.ancestors().len(), true)?;
            uses.extend(path);
        }
        let complete = matches!(
            target,
            SocPhysicsVoxelTarget::Absent
                | SocPhysicsVoxelTarget::Path(SocGeometryTarget::InvalidSelector)
        );
        Ok(sources.value(SourceValue::Present(target), &uses, complete))
    }

    fn field_uses<T>(
        &self,
        field: &SocPhysicsField<'_, T>,
        sources: &SourceUses,
        role: SourceUseRole,
    ) -> Result<Vec<SourceUse>, SocLocationGridError> {
        let element = field.element();
        self.require_owner(element)?;
        let scope = sources.observe(SourceInput::Archive(self.origin), InputScope::Payload)?;
        let location = SourceLocation::xml_element(&element.position(), "$element")
            .ok_or(SocLocationGridError::Position)?;
        let mut uses = vec![scope.at(location, role)?];
        for attribute in field.attributes() {
            let location = SourceLocation::xml_attribute(attribute.position(), attribute.name())
                .ok_or(SocLocationGridError::Position)?;
            uses.push(scope.at(location, role)?);
        }
        Ok(uses)
    }

    fn require_owner(
        &self,
        element: p4kx_extraction::cryxml::SourceElement<'_>,
    ) -> Result<(), SocLocationGridError> {
        if self.descriptor.document().owns_element(element) {
            Ok(())
        } else {
            Err(SocLocationGridError::Owner)
        }
    }
}

/// Outcome of a directory-only lookup of the authored voxel asset.
#[derive(Debug)]
#[non_exhaustive]
pub enum SocPhysicsVoxelTarget {
    /// The source element has no filename.
    Absent,
    /// The selector's lookup state and exact original candidates, without reading their bodies.
    Path(SocGeometryTarget),
}

/// An observed location-grid operation could not be proved.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocLocationGridError {
    /// The original XML element or selected descriptor has a different owner.
    #[error("location-grid evidence belongs to another descriptor")]
    Owner,
    /// A target lookup was requested for an unrelated declaration.
    #[error("location-grid target lookup requires an authored voxel")]
    NotVoxel,
    /// The original XML coordinate cannot be represented.
    #[error("the location-grid XML position cannot be represented")]
    Position,
    /// Archive lookup or cancellation failure.
    #[error(transparent)]
    Capture(#[from] SocCaptureError),
    /// Source registry failure.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::{
    Archive, EntryOrigin, FieldOrigin, FieldPath, InputScope, ModelArchiveSource,
    OriginArtifactBinding, OriginOutputAddress, SourceCoordinateOmission, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    write_origin_companion,
};
use p4kx_model::MeshOrigin;

/// Collects the original mesh geometry and opened Container inputs of one emitted GLB.
pub struct GlbMeshOrigins {
    uses: SourceUses,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    contributors: Vec<SourceUse>,
}

impl Default for GlbMeshOrigins {
    fn default() -> Self {
        Self::new()
    }
}

impl GlbMeshOrigins {
    /// Starts a field-origin capture for one GLB.
    #[must_use]
    pub fn new() -> Self {
        Self {
            uses: SourceUses::with_compact_use_refs(),
            fields: Vec::new(),
            contributors: Vec::new(),
        }
    }

    /// Retains original Container inputs that were opened to build the scene.
    ///
    /// # Errors
    /// Returns an error if a source observation cannot be registered.
    pub fn observe_containers(&mut self, opened: &[Archive]) -> io::Result<()> {
        for entry in opened.iter().filter_map(Archive::container_input_origin) {
            let source = self
                .payload(&entry)?
                .at_opaque(
                    SourceUseRole::Conversion,
                    SourceCoordinateOmission::Unavailable,
                )
                .map_err(io::Error::other)?;
            self.contributors.push(source);
        }
        Ok(())
    }

    /// Captures the geometry contributor to an emitted POSITION accessor count.
    /// The result remains incomplete when the full set of emission operands is unavailable.
    ///
    /// # Errors
    /// Returns an error if an address or observation cannot be registered.
    pub fn observe_mesh(
        &mut self,
        accessor: usize,
        geometry: Option<MeshOrigin>,
        sources: &[ModelArchiveSource],
    ) -> io::Result<()> {
        let location = geometry.and_then(|geometry| {
            let entry = sources
                .iter()
                .find(|source| source.file() == geometry.file)?
                .origin()?;
            let location = SourceLocation::Subresource {
                namespace: "modelChunk".into(),
                indices: vec![geometry.chunk as u64],
                field: "geometry".into(),
            };
            Some((entry, location))
        });
        let contribution = location
            .map(|(entry, location)| {
                self.payload(entry)?
                    .at(location, SourceUseRole::Conversion)
                    .map_err(io::Error::other)
            })
            .transpose()?;
        let count_origin = self.uses.origin(contribution.as_slice(), false);
        self.contributors.extend(contribution);
        let path =
            FieldPath::parse(&format!("/accessors/{accessor}/count")).map_err(io::Error::other)?;
        self.fields
            .push((OriginOutputAddress::GlbJsonField { path }, count_origin));
        Ok(())
    }

    fn payload(&self, entry: &EntryOrigin) -> io::Result<SourceScope> {
        self.uses
            .observe(SourceInput::Archive(entry), InputScope::Payload)
            .map_err(io::Error::other)
    }

    /// Freezes the addressed observations and the partial artifact summary.
    ///
    /// # Errors
    /// Returns an error if the physical evidence cannot be frozen.
    pub fn finish(self) -> io::Result<FrozenGlbMeshOrigins> {
        let artifact = self.uses.origin(&self.contributors, false);
        Ok(FrozenGlbMeshOrigins {
            inventory: self.uses.freeze().map_err(io::Error::other)?,
            artifact,
            fields: self.fields,
        })
    }
}

/// Observed GLB geometry fields and their shared physical origin inventory.
#[derive(Debug)]
pub struct FrozenGlbMeshOrigins {
    inventory: SourceInventory,
    artifact: FieldOrigin,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl FrozenGlbMeshOrigins {
    /// Writes a digest-bound companion containing the observed fields.
    ///
    /// # Errors
    /// Returns an error if the companion cannot be serialized or written.
    pub fn write_companion(
        &self,
        writer: &mut dyn io::Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        write_origin_companion(
            writer,
            binding,
            &self.inventory,
            std::iter::once((&OriginOutputAddress::OpaquePayload, &self.artifact)).chain(
                self.fields
                    .iter()
                    .map(|(address, origin)| (address, origin)),
            ),
        )
    }
}

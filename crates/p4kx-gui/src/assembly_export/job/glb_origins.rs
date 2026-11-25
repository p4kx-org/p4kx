// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::model::MeshOrigin;
use p4kx_extraction::{
    Archive, EntryOrigin, FieldOrigin, FieldPath, InputScope, ModelArchiveSource,
    OriginArtifactBinding, OriginOutputAddress, SourceCoordinateOmission, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    write_origin_companion,
};

pub(super) struct Origins {
    uses: SourceUses,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    contributors: Vec<SourceUse>,
}

impl Origins {
    pub(super) fn new() -> Self {
        Self {
            uses: SourceUses::with_compact_use_refs(),
            fields: Vec::new(),
            contributors: Vec::new(),
        }
    }

    pub(super) fn containers(&mut self, opened: &[Archive]) -> io::Result<()> {
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

    pub(super) fn mesh(
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

    pub(super) fn finish(self) -> io::Result<FrozenOrigins> {
        let artifact = self.uses.origin(&self.contributors, false);
        Ok(FrozenOrigins {
            inventory: self.uses.freeze().map_err(io::Error::other)?,
            artifact,
            fields: self.fields,
        })
    }
}

pub(super) struct FrozenOrigins {
    inventory: SourceInventory,
    artifact: FieldOrigin,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl FrozenOrigins {
    pub(super) fn write_companion(
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

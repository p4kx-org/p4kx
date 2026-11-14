// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::model::{MaterialLibrary, MaterialReadField as Field};
use p4kx_extraction::{InputScope, MaterialEncodingInfo, SourceUseRole as Role};

use crate::material_part_batch::{Band, Batch};
use crate::material_parts::{
    MaterialLibraryEnd, MaterialLibraryHeader, MaterialPart, MaterialTree,
};
use crate::material_parts_origins::{capture, payload, prepared};
use crate::material_payload::StreamMaterialWire;
use crate::material_read_origins::MaterialReadOrigins;
use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{child, independent};
use crate::{MediaError, MediaIssue, MediaWriter, SourceContext, Status, Subresource};

#[path = "material_parts_surface.rs"]
mod surface;
#[path = "material_parts_tree.rs"]
mod tree;

pub(crate) const HEADER: &str = "material-library-header";
pub(crate) const END: &str = "material-library-end";

pub(crate) fn write(
    context: &SourceContext<'_>,
    library: &MaterialLibrary,
    reads: MaterialReadOrigins,
    complete: bool,
    issues: &[MediaIssue],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut output = Output {
        context,
        input: context.origin_input(writer)?,
        reads,
        encoding: library.authored.encoding.into(),
        parts: Batch::new(context, Band::Part),
    };
    output.header(library, complete, issues, writer)?;
    output.authored(library, writer)?;
    for (index, material) in library.materials.iter().enumerate() {
        output.material(index, material, writer)?;
    }
    let parts = output.parts.finish(writer)?;
    let references = crate::material_reference::project(library, context, writer)?;
    let shader_references = crate::material_shader_reference::project(library, context, writer)?;
    let end = MaterialLibraryEnd {
        parts,
        references,
        shader_references,
    };
    let mut origins = prepared(context, writer)?;
    let receipt = capture(
        &output.input,
        InputScope::Payload,
        Some(OriginSite::new(
            "materialXmlDocument",
            &[],
            "libraryProjection",
        )),
        Role::Conversion,
        writer,
    )?;
    origins.insert_subtree(&payload()?, receipt, writer)?;
    writer.record_with_origins(
        &Subresource::new(END, Vec::new()),
        Status::Complete,
        &StreamMaterialWire::LibraryEnd(end),
        &[],
        origins,
    )
}

struct Output<'call, 'source> {
    context: &'call SourceContext<'source>,
    input: MediaOriginInput,
    reads: MaterialReadOrigins,
    encoding: MaterialEncodingInfo,
    parts: Batch<'call, 'source>,
}

impl Output<'_, '_> {
    fn emit(
        &mut self,
        part: &MaterialPart<'_>,
        origins: PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.parts.push(part, origins, writer)
    }

    fn observed(
        &mut self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &p4kx_extraction::FieldPath,
        material: usize,
        fields: &[(&str, Field)],
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (field, source) in fields {
            let origin = self.reads.take(material, *source, &self.input, writer)?;
            prepared.insert_subtree(&child(prefix, field)?, origin, writer)?;
        }
        Ok(())
    }

    fn header(
        &mut self,
        library: &MaterialLibrary,
        complete: bool,
        issues: &[MediaIssue],
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let data = StreamMaterialWire::LibraryHeader(MaterialLibraryHeader {
            path: Cow::Borrowed(&library.path),
            single: library.single,
            default_palette: library.default_palette.as_deref().map(Cow::Borrowed),
            unknown_entries: library.unknown_entries,
            encoding: self.encoding,
            authored_root: library.authored.root.is_some(),
            materials: library.materials.len(),
            source_entries: library.authored.entries.len(),
            source_losses: library.authored.losses.len(),
            projection_complete: complete,
        });
        let origins = self.header_origins(issues, writer)?;
        writer.record_with_origins(
            &Subresource::new(HEADER, Vec::new()),
            if complete {
                Status::Complete
            } else {
                Status::Partial
            },
            &data,
            issues,
            origins,
        )
    }

    fn header_origins(
        &mut self,
        issues: &[MediaIssue],
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut origins = prepared(self.context, writer)?;
        let prefix = payload()?;
        self.observed(
            &mut origins,
            &prefix,
            0,
            &[
                ("single", Field::LibrarySingle),
                ("default_palette", Field::LibraryDefaultPalette),
                ("materials", Field::LibraryTotal),
                ("unknown_entries", Field::LibraryUnknownCount),
            ],
            writer,
        )?;
        let path = capture(
            &self.input,
            InputScope::ArchiveDirectoryMetadata,
            Some(OriginSite::new("media-physical-input", &[], "path")),
            Role::Payload,
            writer,
        )?;
        origins.insert_subtree(&child(&prefix, "path")?, path, writer)?;
        let encoding = capture(
            &self.input,
            InputScope::Payload,
            Some(OriginSite::new("materialXmlDocument", &[], "encoding")),
            Role::Conversion,
            writer,
        )?;
        origins.insert_subtree(&child(&prefix, "encoding")?, encoding, writer)?;
        let projection = capture(
            &self.input,
            InputScope::Payload,
            Some(OriginSite::new("materialXmlDocument", &[], "coverage")),
            Role::Conversion,
            writer,
        )?;
        for field in [
            "authored_root",
            "source_entries",
            "source_losses",
            "projection_complete",
        ] {
            origins.insert_subtree(&child(&prefix, field)?, projection.clone(), writer)?;
        }
        let issues_path = p4kx_extraction::FieldPath::parse("/issues")
            .map_err(|cause| crate::origin_error::address(&cause))?;
        if issues.is_empty() {
            independent(&mut origins, &issues_path, writer)?;
        } else {
            origins.insert_subtree(&issues_path, projection, writer)?;
        }
        Ok(origins)
    }

    fn authored(
        &mut self,
        library: &MaterialLibrary,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let source = &library.authored;
        for (position, entry) in source.entries.iter().enumerate() {
            let indices = source
                .submaterial_list_ordinal
                .map(|list| {
                    Ok::<_, MediaError>([
                        crate::shader_values::count(list)?,
                        crate::shader_values::count(entry.ordinal)?,
                    ])
                })
                .transpose()?;
            let site = indices
                .as_ref()
                .map(|indices| OriginSite::new("materialXmlElement", indices, "elementKind"));
            self.source_part(
                &MaterialPart::SourceEntry {
                    position,
                    entry: entry.into(),
                },
                "entry",
                site,
                Role::Selector,
                writer,
            )?;
        }
        for (position, loss) in source.losses.iter().enumerate() {
            let indices = loss
                .indices
                .iter()
                .copied()
                .map(crate::shader_values::count)
                .collect::<Result<Vec<_>, _>>()?;
            let site = (!matches!(
                loss.kind,
                p4kx_extraction::model::MaterialSourceLossKind::RepairedEncoding
            ))
            .then_some(OriginSite::new(
                "materialXmlElement",
                &indices,
                "captureBoundary",
            ));
            self.source_part(
                &MaterialPart::SourceLoss {
                    position,
                    loss: loss.into(),
                },
                "loss",
                site,
                Role::Conversion,
                writer,
            )?;
        }
        if let Some(root) = &source.root {
            self.tree(MaterialTree::Library, root, Some(&[]), writer)?;
        }
        Ok(())
    }

    fn source_part(
        &mut self,
        part: &MaterialPart<'_>,
        field: &str,
        site: Option<OriginSite<'_>>,
        role: Role,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut origins = self.part_origins(writer)?;
        let origin = capture(&self.input, InputScope::Payload, site, role, writer)?;
        origins.insert_subtree(&child(&payload()?, field)?, origin, writer)?;
        self.emit(part, origins, writer)
    }

    fn part_origins(&self, writer: &MediaWriter<'_>) -> Result<PreparedRecordOrigins, MediaError> {
        let mut origins = prepared(self.context, writer)?;
        independent(&mut origins, &payload()?, writer)?;
        Ok(origins)
    }
}

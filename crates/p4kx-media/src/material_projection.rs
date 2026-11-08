// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits,
};
use p4kx_extraction::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};

use crate::material_payload::{
    MaterialFailureReason, MaterialLibraryMetadata, MaterialMetadata, MaterialPayload,
    MaterialProjectionFailure,
};
use crate::material_surface::MaterialSurface;
use crate::{
    MediaError, MediaIssue, MediaWriter, Projection, Severity, SourceContext, Status, Subresource,
};
use crate::{material_reference, retained_payload};

pub(crate) const LIBRARY_NAMESPACE: &str = "material-library";
pub(crate) const FAILURE_NAMESPACE: &str = "material-library-failure";

pub(crate) fn project(
    context: &SourceContext<'_>,
    limits: MaterialSourceLimits,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let bound = count(limits.source_bytes)?;
    if context.source.size > bound {
        return failed(MaterialFailureReason::SourceBytes { limit: bound }, writer);
    }
    let bytes = retained_payload::read(context, writer)?;
    let library = match MaterialLibrary::parse_with(context.entry.path(), &bytes, limits, || {
        writer.checkpoint()
    }) {
        Ok(library) => library,
        Err(MaterialReadError::Interrupted(error)) => return Err(error),
        Err(MaterialReadError::Material(error)) => return failed(reason(&error)?, writer),
        Err(_) => return failed(MaterialFailureReason::UnsupportedReader, writer),
    };
    writer.checkpoint()?;
    let authored = MaterialLibrarySourceInfo::of(&library);
    let materials: Vec<_> = library
        .materials
        .iter()
        .enumerate()
        .map(|(index, material)| MaterialMetadata {
            index,
            name: Cow::Borrowed(&material.name),
            authored: MaterialAuthoredInfo::of(material),
            interpreted_surface: MaterialSurface::of(material),
        })
        .collect();
    let complete = authored.is_complete()
        && materials
            .iter()
            .all(|material| material.authored.is_complete());
    let issues = diagnoses(&library, complete);
    let references = material_reference::project(&library, context, writer)?;
    let shader_references = crate::material_shader_reference::project(&library, context, writer)?;
    let metadata = MaterialLibraryMetadata {
        path: Cow::Borrowed(&library.path),
        single: library.single,
        default_palette: library.default_palette.as_deref().map(Cow::Borrowed),
        unknown_entries: library.unknown_entries,
        authored,
        materials,
        references,
        shader_references,
        projection_complete: complete,
    };
    let payload = MaterialPayload::Library(&metadata);
    writer.record(
        &Subresource::new(LIBRARY_NAMESPACE, Vec::new()),
        if complete {
            Status::Complete
        } else {
            Status::Partial
        },
        &payload,
        &issues,
    )?;
    Ok(Projection::Written)
}

fn diagnoses(library: &MaterialLibrary, complete: bool) -> Vec<MediaIssue> {
    let mut issues = Vec::new();
    for loss in &library.authored.losses {
        issues.push(MediaIssue::new("material.source.projection-loss", format!("authored.elements{:?}", loss.indices), Severity::Warning, format!("Structured source boundary: {:?}; original decoded input bytes remain separately retained.", loss.kind)));
    }
    let materials = library.materials.iter().enumerate();
    for (index, material) in materials {
        for cut in &material.truncated {
            issues.push(MediaIssue::new(
                "material.fields.truncated",
                format!("materials[{index}].authored"),
                Severity::Warning,
                cut,
            ));
        }
    }
    if !complete && issues.is_empty() {
        issues.push(MediaIssue::new("material.source.unsupported-reader-value", "authored", Severity::Warning, "An unsupported reader value prevents a complete structured source projection; exact input retention is separate."));
    }
    issues
}

fn reason(error: &MaterialError) -> Result<MaterialFailureReason, MediaError> {
    Ok(match error {
        MaterialError::TextEncoding(_) => MaterialFailureReason::TextEncoding,
        MaterialError::Xml(_) => MaterialFailureReason::Xml,
        MaterialError::CryXml(_) => MaterialFailureReason::CryXml,
        MaterialError::TooDeep(_) => MaterialFailureReason::XmlDepth,
        MaterialError::NotMaterial { .. } => MaterialFailureReason::NotMaterial,
        MaterialError::SourceBytes { limit } => MaterialFailureReason::SourceBytes {
            limit: count(*limit)?,
        },
        MaterialError::TooMany { what, limit } => MaterialFailureReason::TypedLimit {
            what: (*what).to_owned(),
            limit: count(*limit)?,
        },
        MaterialError::SourceSpan => MaterialFailureReason::SourceSpan,
        MaterialError::InvalidSourceLimits => MaterialFailureReason::InvalidLimits,
        _ => MaterialFailureReason::UnsupportedReader,
    })
}

fn count(value: usize) -> Result<u64, MediaError> {
    u64::try_from(value)
        .map_err(|_| MediaError::Contract("material source count exceeds unsigned64".into()))
}

fn failed(
    reason: MaterialFailureReason,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let data = MaterialPayload::Failure(MaterialProjectionFailure { reason });
    writer.record(&Subresource::new(FAILURE_NAMESPACE, Vec::new()), Status::Failed, &data,
        &[MediaIssue::new("material.source.failed", "material", Severity::Error, "Structured material projection failed; the common source envelope identifies the independently retained original decoded bytes.")])?;
    Ok(Projection::Written)
}

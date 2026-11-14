// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialReadObservation,
    MaterialSourceLimits,
};
use p4kx_extraction::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};

use crate::material_payload::{MaterialFailureReason, MaterialPayload, MaterialProjectionFailure};

use crate::retained_payload;
use crate::{
    MediaError, MediaIssue, MediaWriter, Projection, Severity, SourceContext, Status, Subresource,
};

pub(crate) const LIBRARY_NAMESPACE: &str = "material-library";
pub(crate) const FAILURE_NAMESPACE: &str = "material-library-failure";

pub(crate) fn project(
    context: &SourceContext<'_>,
    limits: MaterialSourceLimits,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    project_observed(context, limits, writer, |_| Ok(()))
}

pub(crate) fn project_observed(
    context: &SourceContext<'_>,
    limits: MaterialSourceLimits,
    writer: &mut MediaWriter<'_>,
    mut before_capture: impl FnMut(MaterialReadObservation<'_>) -> Result<(), MediaError>,
) -> Result<Projection, MediaError> {
    let bound = count(limits.source_bytes)?;
    if context.source.size > bound {
        return failed(
            context,
            MaterialFailureReason::SourceBytes { limit: bound },
            writer,
        );
    }
    let bytes = retained_payload::read(context, writer)?;
    let mut reads = crate::material_read_origins::MaterialReadOrigins::default();
    let detail_bytes = writer.limits().record_bytes;
    let library = match MaterialLibrary::parse_with_observer(
        context.entry.path(),
        &bytes,
        limits,
        || writer.checkpoint(),
        |observation| {
            before_capture(observation)?;
            writer.checkpoint()?;
            reads.observe(observation, detail_bytes);
            Ok(())
        },
    ) {
        Ok(library) => library,
        Err(MaterialReadError::Interrupted(error)) => return Err(error),
        Err(MaterialReadError::Material(error)) => return failed(context, reason(&error)?, writer),
        Err(_) => return failed(context, MaterialFailureReason::UnsupportedReader, writer),
    };
    write_library(context, &library, reads, writer)
}

fn write_library(
    context: &SourceContext<'_>,
    library: &MaterialLibrary,
    reads: crate::material_read_origins::MaterialReadOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let mut check = || writer.checkpoint();
    let mut complete =
        MaterialLibrarySourceInfo::source_is_complete_with(&library.authored, &mut check)?;
    for material in &library.materials {
        complete &= MaterialAuthoredInfo::material_is_complete_with(material, &mut check)?;
    }
    let issues = diagnoses(library, complete);
    crate::material_parts_write::write(context, library, reads, complete, &issues, writer)?;
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
    context: &SourceContext<'_>,
    reason: MaterialFailureReason,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let origins = crate::origin_material::failure(context, &reason, writer)?;
    let data = MaterialPayload::Failure(MaterialProjectionFailure { reason });
    writer.record_with_origins(&Subresource::new(FAILURE_NAMESPACE, Vec::new()), Status::Failed, &data,
        &[MediaIssue::new("material.source.failed", "material", Severity::Error, "Structured material projection failed; the common source envelope identifies the independently retained original decoded bytes.")], origins)?;
    Ok(Projection::Written)
}

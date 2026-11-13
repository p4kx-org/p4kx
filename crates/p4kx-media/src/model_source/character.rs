// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{SourceElement, SourceReadError};
use p4kx_extraction::model::character_definition::{
    CharacterDefinition, relative_position, relative_rotation,
};

use super::character_origin::CharacterElement;
use super::character_payload::{
    self as wire, Attachment, BoneOffset, CharacterRecord, Definition, Skeleton,
};
use super::character_reference::CharacterReferences;
use super::origins::{self, ModelOrigins};
use crate::{
    MediaError, MediaIssue, MediaWriter, Projection, Severity, SourceContext, Status, Subresource,
    required_reader,
};

pub(super) fn emit(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let input = context.origin_input(writer)?;
    let origins = ModelOrigins {
        input: &input,
        entry: context.entry.index(),
    };
    let bytes = required_reader::read_all(context, writer)?;
    let budget = &context.directory.input_state()?.budget;
    budget.metadata(0, bytes.len() as u64, writer.limits(), writer.checkpoint)?;
    let parsed = CharacterDefinition::read(&bytes);
    write_definition(&parsed, bytes.len(), &origins, writer)?;
    if let Ok(source) = parsed {
        let references = CharacterReferences {
            context,
            origins: &origins,
        };
        for (index, model) in source.models() {
            writer.checkpoint()?;
            write_skeleton(index, model, &references, writer)?;
        }
        for (indices, attachment) in source.attachments() {
            writer.checkpoint()?;
            write_attachment(indices, attachment, &references, writer)?;
        }
    }
    Ok(Projection::Written)
}

fn write_definition(
    parsed: &Result<CharacterDefinition<'_>, SourceReadError>,
    length: usize,
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins.prepared()?;
    let evidence = origins.payload(
        0..length as u64,
        "characterDefinition",
        SourceUseRole::Payload,
        writer,
    )?;
    origins::at(&mut prepared, "/data/payload", evidence.clone(), writer)?;
    if let Ok(source) = parsed {
        let root = CharacterElement::new(source.root(), origins).element_origin("root", writer)?;
        for field in ["/data/payload/root_element", "/data/payload/disposition"] {
            origins::at(&mut prepared, field, root.clone(), writer)?;
        }
    }
    let issue = match parsed {
        Err(error) => Some(MediaIssue::new(
            "model.character-definition",
            "entry",
            Severity::Warning,
            error.to_string(),
        )),
        Ok(source) if source.is_lossy() => Some(MediaIssue::new(
            "model.character-strings",
            "entry",
            Severity::Warning,
            "CDF contains repaired CryXml string bytes",
        )),
        Ok(_) => None,
    };
    let status = if issue.is_some() {
        origins::at(&mut prepared, "/status", evidence.clone(), writer)?;
        origins::at(&mut prepared, "/issues", evidence, writer)?;
        Status::Partial
    } else {
        Status::Complete
    };
    prepared.write(
        writer,
        &Subresource::new(wire::DOCUMENT, Vec::new()),
        status,
        &CharacterRecord::Definition(Definition::new(parsed.as_ref().ok())),
        issue.as_slice(),
    )
}

fn write_skeleton(
    ordinal: usize,
    element: SourceElement<'_>,
    references: &CharacterReferences<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let model = CharacterElement::new(element, references.origins);
    let mut prepared = model.prepare(writer)?;
    let file = references.field(&model, "File", "/data/payload/file", &mut prepared, writer)?;
    prepared.write(
        writer,
        &Subresource::new(wire::SKELETON, vec![ordinal as u64]),
        Status::Complete,
        &CharacterRecord::Skeleton(Skeleton { file }),
        &[],
    )
}

fn write_attachment(
    indices: [usize; 2],
    element: SourceElement<'_>,
    references: &CharacterReferences<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let attachment = CharacterElement::new(element, references.origins);
    let mut prepared = attachment.prepare(writer)?;
    let mut original = |name, pointer| attachment.field(name, pointer, &mut prepared, writer);
    let name = original("AName", "/data/payload/name")?;
    let material = original("Material", "/data/payload/material")?;
    let bone_name = original("BoneName", "/data/payload/bone_name")?;
    let (attachment_type, kind_origin) = attachment.value("Type", writer)?;
    origins::at(
        &mut prepared,
        "/data/payload/attachment_type",
        kind_origin.clone(),
        writer,
    )?;
    let is_bone = attachment_type.is_some_and(|kind| kind.eq_ignore_ascii_case("CA_BONE"));
    let mut offset =
        |name, pointer| attachment.offset(name, pointer, &kind_origin, &mut prepared, writer);
    let position = offset("RelPosition", "/data/payload/relative_position")?;
    let rotation = offset("RelRotation", "/data/payload/relative_rotation_wxyz")?;
    let binding = references.field(
        &attachment,
        "Binding",
        "/data/payload/binding",
        &mut prepared,
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(wire::ATTACHMENT, indices.map(|value| value as u64).to_vec()),
        Status::Complete,
        &CharacterRecord::Attachment(Attachment {
            name: name.map(str::to_owned),
            kind: attachment_type.map(str::to_owned),
            binding,
            material: material.map(str::to_owned),
            bone_name: bone_name.map(str::to_owned),
            relative_position: BoneOffset {
                authored: position.map(str::to_owned),
                value: is_bone.then(|| relative_position(position)).flatten(),
            },
            relative_rotation_wxyz: BoneOffset {
                authored: rotation.map(str::to_owned),
                value: is_bone.then(|| relative_rotation(rotation)).flatten(),
            },
        }),
        &[],
    )
}

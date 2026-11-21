// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::cryxml::{SourceElement, SourceReadError};
use p4kx_extraction::model::character_definition::{self, CharacterDefinition};

use super::character_offsets::CharacterOffsets;
use super::character_origin::CharacterElement;
use super::character_payload::{self as wire, Attachment, CharacterRecord, Definition, Skeleton};
use super::character_reference::CharacterReferences;
use super::character_rig::CharacterRig;
use super::character_rig_placement::{self, AttachmentPose};
use super::origins::{self, ModelOrigins};
use crate::origin_prepared::PreparedRecordOrigins;
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
            strings_lossy: source.is_lossy(),
        };
        for (index, model) in source.models() {
            writer.checkpoint()?;
            write_skeleton(index, model, &origins, &references, writer)?;
        }
        let first_bone = first_bone_attachment(&source, writer)?;
        let rig = first_bone
            .map(|indices| CharacterRig::read(&source, &origins, &references, indices, writer))
            .transpose()?;
        for (indices, attachment) in source.attachments() {
            writer.checkpoint()?;
            write_attachment(
                indices,
                attachment,
                &origins,
                &references,
                rig.as_ref(),
                writer,
            )?;
        }
    }
    Ok(Projection::Written)
}

fn first_bone_attachment(
    source: &CharacterDefinition<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Option<[usize; 2]>, MediaError> {
    for (indices, attachment) in source.attachments() {
        writer.checkpoint()?;
        if character_definition::attribute(attachment, "Type")
            .is_some_and(|kind| character_rig_placement::is_bone(kind.value()))
        {
            return Ok(Some(indices));
        }
    }
    Ok(None)
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
        &CharacterRecord::Definition(&Definition::new(parsed.as_ref().ok())),
        issue.as_slice(),
    )
}

fn write_skeleton(
    ordinal: usize,
    element: SourceElement<'_>,
    origins: &ModelOrigins<'_>,
    references: &CharacterReferences<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let model = CharacterElement::new(element, origins);
    let mut prepared = model.prepare(writer)?;
    let file = references.field(&model, "File", "/data/payload/file", &mut prepared, writer)?;
    prepared.write(
        writer,
        &Subresource::new(wire::SKELETON, vec![ordinal as u64]),
        Status::Complete,
        &CharacterRecord::Skeleton(&Skeleton { file }),
        &[],
    )
}

fn write_attachment(
    indices: [usize; 2],
    element: SourceElement<'_>,
    origins: &ModelOrigins<'_>,
    references: &CharacterReferences<'_, '_>,
    rig: Option<&CharacterRig>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let attachment = CharacterElement::new(element, origins);
    let mut prepared = attachment.prepare(writer)?;
    let payload = attachment_payload(&attachment, references, rig, &mut prepared, writer)?;
    prepared.write(
        writer,
        &Subresource::new(wire::ATTACHMENT, indices.map(|value| value as u64).to_vec()),
        Status::Complete,
        &CharacterRecord::Attachment(&payload),
        &[],
    )
}

fn attachment_payload(
    element: &CharacterElement<'_, '_>,
    references: &CharacterReferences<'_, '_>,
    rig: Option<&CharacterRig>,
    prepared: &mut PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<Attachment, MediaError> {
    let mut field = |name, path| {
        element
            .field(name, path, prepared, writer)
            .map(|value| value.map(str::to_owned))
    };
    let name = field("AName", "/data/payload/name")?;
    let bone = field("BoneName", "/data/payload/bone_name")?;
    let (kind, kind_origin) = element.value("Type", writer)?;
    origins::at(
        prepared,
        "/data/payload/attachment_type",
        kind_origin.clone(),
        writer,
    )?;
    let is_bone = kind.is_some_and(character_rig_placement::is_bone);
    let offsets = CharacterOffsets::read(element, is_bone, &kind_origin, prepared, writer)?;
    let links = references.attachment(element, prepared, writer)?;
    let rig_placement = if is_bone {
        let rig = rig.ok_or_else(|| {
            MediaError::Contract("CDF bone attachment has no rig selection".into())
        })?;
        let authored = AttachmentPose {
            element,
            kind: &kind_origin,
            bone: bone.as_deref(),
            decoded: offsets.decoded,
            strings_lossy: references.strings_lossy,
        };
        Some(rig.write(&authored, prepared, writer)?)
    } else {
        None
    };
    Ok(Attachment {
        name,
        kind: kind.map(str::to_owned),
        binding: links.binding,
        material: links.material.map(str::to_owned),
        material_reference: links.material_reference,
        bone_name: bone,
        relative_position: offsets.relative_position,
        relative_rotation_wxyz: offsets.relative_rotation,
        model_position: offsets.model_position,
        model_rotation_wxyz: offsets.model_rotation,
        rig_placement,
    })
}

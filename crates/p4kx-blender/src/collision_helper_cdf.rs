// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    AttachedModel, BonePlacementSource, CdfAttachmentField, CdfAttachmentSite, Part,
};
use p4kx_extraction::cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_extraction::{
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use p4kx_model::ModelFile;
use p4kx_model::character_definition::{CharacterDefinition, attribute};

use crate::PackageResult;
use crate::collision_helper_document::{CdfPlacement, SelectedRig};
use crate::collision_helper_model::{OriginalModels, error};

pub(super) fn capture(
    owner: &Part,
    ordinal: usize,
    attachment: &AttachedModel,
    bytes: &[u8],
    input: &OriginalModels<'_, '_>,
) -> PackageResult<Option<CdfPlacement>> {
    let Some(definition) = attachment.model_definition_origin() else {
        return Ok(None);
    };
    let document = CharacterDefinition::read(bytes).map_err(error)?;
    let scope = input.observe(definition)?;
    let mut attachment_uses = Vec::new();
    for field in attachment.cdf_sources() {
        if field.field() == CdfAttachmentField::Material {
            continue;
        }
        if let Some(location) = field.location() {
            let role = if field.field().is_offset() {
                SourceUseRole::Conversion
            } else {
                SourceUseRole::Selector
            };
            attachment_uses.push(scope.at(location, role).map_err(error)?);
        }
    }
    let matched_rig = owner.attachment_bone_placement_source(ordinal);
    let selected_rig = matched_rig
        .as_ref()
        .map(|rig| bind_rig(rig, input, &mut attachment_uses))
        .transpose()?;
    let proven = selected_rig.is_some() && attachment.offset_used();
    let frame = if proven {
        attachment
            .local()
            .finite()
            .map_or(SourceValue::Invalid, |value| {
                SourceValue::Present(value.rows)
            })
    } else {
        SourceValue::Unsupported
    };
    let bone = attachment
        .cdf_sources()
        .iter()
        .find(|site| site.field() == CdfAttachmentField::BoneName)
        .and_then(CdfAttachmentSite::location)
        .map(|location| scope.at(location, SourceUseRole::Payload).map_err(error))
        .transpose()?;
    Ok(Some(CdfPlacement {
        definition: scope.source_ref().clone(),
        attachment_bone: input.evidence.value(
            SourceValue::Present(attachment.bone().to_owned()),
            bone.as_slice(),
            bone.is_some(),
        ),
        declared_skeleton_file: declared_skeleton(&document, &scope, input.evidence)?,
        selected_rig,
        owner_from_attachment: input.evidence.value(frame, &attachment_uses, proven),
    }))
}

fn bind_rig(
    rig: &BonePlacementSource<'_>,
    input: &OriginalModels<'_, '_>,
    combined: &mut Vec<SourceUse>,
) -> PackageResult<SelectedRig> {
    let mut uses = Vec::new();
    let scope = input.observe(rig.origin())?;
    if let Some(cdf) = rig.definition_origin() {
        let declared = input.observe(cdf)?;
        for selector in rig.cdf_model_selectors() {
            uses.push(
                declared
                    .at(selector, SourceUseRole::Selector)
                    .map_err(error)?,
            );
        }
    }
    bind_lineage(rig, input, &scope, &mut uses)?;
    let origin = input.evidence.origin(&uses, true);
    combined.extend(uses);
    Ok(SelectedRig {
        source: scope.source_ref().clone(),
        chunk: rig.chunk(),
        bone: rig.bone_ordinal(),
        lod: match rig.file() {
            ModelFile::LodModel(lod) => Some(lod),
            _ => None,
        },
        origin,
    })
}

fn bind_lineage(
    rig: &BonePlacementSource<'_>,
    input: &OriginalModels<'_, '_>,
    scope: &SourceScope,
    uses: &mut Vec<SourceUse>,
) -> PackageResult<()> {
    for (index, bone) in rig.bones() {
        input.check()?;
        let fields = [
            (bone.parent(), "parent", SourceUseRole::Selector),
            (
                bone.relative_pose(),
                "relativePose",
                SourceUseRole::Conversion,
            ),
        ];
        for (range, field, role) in fields {
            uses.push(
                scope
                    .at(
                        SourceLocation::Subresource {
                            namespace: "model-skeleton-bone".into(),
                            indices: vec![rig.chunk() as u64, index as u64, range.start, range.end],
                            field: field.into(),
                        },
                        role,
                    )
                    .map_err(error)?,
            );
        }
        if index == rig.bone_ordinal() {
            let range = bone.name();
            uses.push(
                scope
                    .at(
                        SourceLocation::Subresource {
                            namespace: "model-skeleton-bone".into(),
                            indices: vec![rig.chunk() as u64, index as u64, range.start, range.end],
                            field: "name".into(),
                        },
                        SourceUseRole::Selector,
                    )
                    .map_err(error)?,
            );
        }
    }
    Ok(())
}

fn declared_skeleton(
    document: &CharacterDefinition<'_>,
    scope: &SourceScope,
    uses: &SourceUses,
) -> PackageResult<SourcedValue<String>> {
    if let Some((_, declaration)) = document.models().next()
        && let Some(original) = attribute(declaration, "File")
    {
        let (namespace, indices) = match original.position() {
            SourceAttributePosition::Text(range) => (
                "cdfModelTextBytes",
                vec![range.start as u64, range.end as u64],
            ),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => (
                "cdfModelCryXmlAttribute",
                vec![u64::from(*node_index), u64::from(*attribute_index)],
            ),
            _ => {
                return Ok(uses.value(
                    SourceValue::Present(original.value().to_owned()),
                    &[],
                    false,
                ));
            }
        };
        let field = scope
            .at(
                SourceLocation::Subresource {
                    namespace: namespace.into(),
                    indices,
                    field: "File".into(),
                },
                SourceUseRole::Payload,
            )
            .map_err(error)?;
        return Ok(uses.value(
            SourceValue::Present(original.value().to_owned()),
            &[field],
            true,
        ));
    }
    let (namespace, indices) = match document.root().position() {
        SourceElementPosition::Text(range) => (
            "cdfDefinitionTextBytes",
            vec![range.start as u64, range.end as u64],
        ),
        SourceElementPosition::CryXml(node) => ("cdfDefinitionCryXmlNode", vec![u64::from(node)]),
        _ => return Ok(uses.value(SourceValue::Absent, &[], false)),
    };
    let use_at = scope
        .at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: "skeletonDeclaration".into(),
            },
            SourceUseRole::Selector,
        )
        .map_err(error)?;
    Ok(uses.value(SourceValue::Absent, &[use_at], true))
}

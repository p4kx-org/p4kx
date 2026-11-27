// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    Assembly, InteriorLightHelper, InteriorLightHelperError, InteriorLightHelperSource,
    InteriorLightPlacement, InteriorPart, ObjectContainerRef, SocTileLightAnchorError,
};
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, SourceInventory, SourceUses, SourceValue, SourcedValue,
};
use p4kx_gltf::NodeTransform;
use p4kx_model::SocLight;
use p4kx_model::Transform;
use schemars::JsonSchema;
use serde::Serialize;

use crate::items::Item;
use crate::soc_scene_origin::source_value;
use crate::{PackageError, PackageResult, PackageWriteContext};

#[path = "package_light_origins.rs"]
mod origins;

use origins::Evidence;

type Frame = [[f32; 4]; 3];

#[derive(Debug)]
pub(crate) struct PlacedLight<'a> {
    pub(crate) owner: usize,
    pub(crate) container: usize,
    pub(crate) reference: &'a ObjectContainerRef,
    pub(crate) part: &'a InteriorPart,
    pub(crate) light: &'a SocLight,
    pub(crate) placement: &'a InteriorLightPlacement,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct LightPlacements {
    rows: Vec<LightPlacement>,
    #[serde(rename = "sourceInventory")]
    inventory: SourceInventory,
}

#[derive(Serialize, JsonSchema)]
struct LightPlacement {
    node: usize,
    owner: usize,
    container: usize,
    preview_state: SourcedValue<String>,
    original_light: SourcedValue<Frame>,
    original_group: SourcedValue<Frame>,
    tile_selector: SourcedValue<String>,
    declared_helper: SourcedValue<String>,
    effective_light: SourcedValue<Frame>,
    anchor: Anchor,
    helper: Helper,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Anchor {
    Undeclared,
    Unresolved {
        reason: &'static str,
        port: Option<Port>,
    },
    Anchored {
        effective_group: SourcedValue<Frame>,
        owner_from_container: SourcedValue<Frame>,
        port: Option<Port>,
    },
}

#[derive(Serialize, JsonSchema)]
struct Port {
    name: SourcedValue<String>,
    interaction_offset: SourcedValue<[f64; 3]>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Helper {
    Undeclared,
    Unresolved {
        reason: &'static str,
    },
    Matched {
        model_node: usize,
        name: SourcedValue<String>,
        owner_from_helper: SourcedValue<Frame>,
        helper_from_light: SourcedValue<Frame>,
        source: Box<HelperSource>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HelperSource {
    Group,
    VehiclePoint {
        part: SourcedValue<String>,
        point: SourcedValue<String>,
        document: SourcedValue<String>,
        origin: &'static str,
    },
    Unknown,
}

pub(crate) struct LightCapture {
    pub(crate) placements: Option<serde_json::Value>,
    pub(crate) node_fields: FieldOrigins,
    pub(crate) sources: SourceUses,
}

/// Captures the baked light placements and the same observed origins of their renderer transforms.
pub(crate) fn capture(
    assembly: &Assembly,
    items: &[Item<'_>],
    context: &PackageWriteContext<'_>,
) -> PackageResult<LightCapture> {
    let sources = SourceUses::with_compact_use_refs();
    let mut rows = Vec::new();
    let mut node_fields = FieldOrigins::new();
    for (node, item) in items.iter().enumerate() {
        if let Some(placed) = &item.light_placement {
            context.check()?;
            let placement = LightPlacement::read(node, placed, assembly, &sources)?;
            if item.light.is_some() && placed.part.parent().is_some() {
                capture_node_transform(
                    &mut node_fields,
                    node,
                    &item.transform,
                    placed.part.local(),
                    placement.effective_light.origin(),
                )?;
            }
            rows.push(placement);
        }
    }
    let placements = if rows.is_empty() {
        None
    } else {
        let inventory = sources.freeze().map_err(error)?;
        Some(serde_json::to_value(LightPlacements { rows, inventory }).map_err(error)?)
    };
    Ok(LightCapture {
        placements,
        node_fields,
        sources,
    })
}

fn capture_node_transform(
    fields: &mut FieldOrigins,
    node: usize,
    transform: &NodeTransform,
    authored: &Transform,
    origin: &FieldOrigin,
) -> PackageResult<()> {
    // Quaternion conversion can produce finite identity from a non-finite basis.
    let valid_basis = authored
        .rows
        .iter()
        .all(|[x, y, z, _]| x.is_finite() && y.is_finite() && z.is_finite());
    let finite_fields = [
        (
            "translation",
            transform.translation.into_iter().all(f32::is_finite),
        ),
        (
            "rotation",
            valid_basis && transform.rotation.into_iter().all(f32::is_finite),
        ),
        ("scale", transform.scale.into_iter().all(f32::is_finite)),
    ];
    let mut path = FieldPath::parse("/nodes").map_err(error)?;
    path.push_index(node).map_err(error)?;
    for (field, finite) in finite_fields {
        if finite {
            path.push_field(field).map_err(error)?;
            fields.insert(path.clone(), origin.clone()).map_err(error)?;
            path.pop();
        }
    }
    Ok(())
}

impl LightPlacement {
    fn read(
        node: usize,
        placed: &PlacedLight<'_>,
        assembly: &Assembly,
        sources: &SourceUses,
    ) -> PackageResult<Self> {
        let group = placed.light.group_placement.as_deref().ok_or_else(|| {
            error(std::io::Error::other(
                "direct baked light lacks its original group",
            ))
        })?;
        let origins = Evidence::read(placed, assembly, sources)?;
        let anchor = Anchor::read(placed.placement, &origins, sources);
        let helper = Helper::read(&placed.placement.helper, &origins, sources);
        Ok(Self {
            node,
            owner: placed.owner,
            container: placed.container,
            preview_state: origins.state.value(
                sources,
                SourceValue::Present(placed.light.state.code().to_owned()),
            ),
            original_light: origins
                .light
                .value(sources, SourceValue::Present(placed.light.transform.rows)),
            original_group: origins
                .group
                .value(sources, SourceValue::Present(group.authored.rows)),
            tile_selector: origins
                .tile
                .value(sources, source_value(group.tile.clone())),
            declared_helper: origins
                .declared_helper
                .value(sources, source_value(group.helper.clone())),
            effective_light: origins
                .effective_light
                .value(sources, SourceValue::Present(placed.part.local().rows)),
            anchor,
            helper,
        })
    }
}

impl Anchor {
    fn read(placement: &InteriorLightPlacement, evidence: &Evidence, sources: &SourceUses) -> Self {
        let port = placement.port.as_ref().map(|port| Port {
            name: evidence
                .port_name
                .value(sources, SourceValue::Present(port.name.clone())),
            interaction_offset: evidence.offset.value(sources, source_value(port.offset)),
        });
        match placement.anchor {
            None => Self::Undeclared,
            Some(Err(reason)) => Self::Unresolved {
                reason: anchor_reason(reason),
                port,
            },
            Some(Ok(anchor)) => Self::Anchored {
                effective_group: evidence
                    .anchor
                    .value(sources, SourceValue::Present(anchor.anchored().rows)),
                owner_from_container: evidence.owner.value(
                    sources,
                    placement
                        .owner_from_container
                        .map_or(SourceValue::Unreadable, |frame| {
                            SourceValue::Present(frame.rows)
                        }),
                ),
                port,
            },
        }
    }
}

impl Helper {
    fn read(helper: &InteriorLightHelper, evidence: &Evidence, sources: &SourceUses) -> Self {
        match helper {
            InteriorLightHelper::Undeclared => Self::Undeclared,
            InteriorLightHelper::Matched(frame) => Self::Matched {
                model_node: frame.node,
                name: evidence
                    .helper_name
                    .value(sources, SourceValue::Present(frame.name.clone())),
                owner_from_helper: evidence
                    .helper_model
                    .value(sources, SourceValue::Present(frame.owner_from_helper.rows)),
                helper_from_light: evidence
                    .helper_frame
                    .value(sources, SourceValue::Present(frame.helper_from_light.rows)),
                source: Box::new(HelperSource::read(
                    &frame.source,
                    &evidence.helper_name,
                    sources,
                )),
            },
            InteriorLightHelper::Unresolved(reason) => Self::Unresolved {
                reason: helper_reason(*reason),
            },
            _ => Self::Unresolved { reason: "unknown" },
        }
    }
}

impl HelperSource {
    fn read(
        source: &InteriorLightHelperSource,
        evidence: &origins::Uses,
        sources: &SourceUses,
    ) -> Self {
        let field = |value: &String| evidence.value(sources, SourceValue::Present(value.clone()));
        match source {
            InteriorLightHelperSource::Group => Self::Group,
            InteriorLightHelperSource::VehiclePoint {
                part,
                point,
                document,
                origin,
                ..
            } => Self::VehiclePoint {
                part: field(part),
                point: field(point),
                document: field(document),
                origin: origin.code(),
            },
            _ => Self::Unknown,
        }
    }
}

const fn anchor_reason(reason: SocTileLightAnchorError) -> &'static str {
    use SocTileLightAnchorError as Error;
    match reason {
        Error::DescriptorUnavailable => "descriptor_unavailable",
        Error::AbsentSelector => "absent_selector",
        Error::InvalidSelector => "invalid_selector",
        Error::UnmatchedSelector => "unmatched_selector",
        Error::AmbiguousSelector => "ambiguous_selector",
        Error::AbsentOffset => "absent_offset",
        Error::InvalidOffset => "invalid_offset",
        Error::MissingContainerFrame => "missing_container_frame",
        Error::InvalidContainerFrame => "invalid_container_frame",
        Error::MissingGroupFrame => "missing_group_frame",
        Error::InvalidGroupFrame => "invalid_group_frame",
        Error::UnrepresentableLight => "unrepresentable_light",
        Error::UnrepresentableAnchor => "unrepresentable_anchor",
        _ => "unknown",
    }
}

const fn helper_reason(reason: InteriorLightHelperError) -> &'static str {
    use InteriorLightHelperError as Error;
    match reason {
        Error::InvalidExplicitHelper => "invalid_explicit_helper",
        Error::TilePortUnavailable => "tile_port_unavailable",
        Error::VehicleUnavailable => "vehicle_unavailable",
        Error::PointUnmatched => "point_unmatched",
        Error::PointAmbiguous => "point_ambiguous",
        Error::PointWithoutBone => "point_without_bone",
        Error::ModelHelperUnavailable => "model_helper_unavailable",
        Error::ModelHelperAmbiguous => "model_helper_ambiguous",
        Error::InvalidFrame => "invalid_frame",
        _ => "unknown",
    }
}

fn error(error: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::LightEvidence(Box::new(error))
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::{FieldOrigins, SourceUses};
    use p4kx_gltf::light_node_transform;
    use p4kx_model::Transform;

    use crate::PackageResult;

    use super::capture_node_transform;

    #[test]
    fn renderer_fields_require_finite_source_vectors() -> PackageResult<()> {
        let sources = SourceUses::with_compact_use_refs();
        let origin = sources.origin(&[], false);
        let [_, y, z] = Transform::IDENTITY.rows;
        let invalid_translation = Transform {
            rows: [[1.0, 0.0, 0.0, f32::NAN], y, z],
        };
        let invalid_basis = Transform {
            rows: [[f32::NAN, 0.0, 0.0, 0.0], y, z],
        };
        let mut fields = FieldOrigins::new();
        for (index, authored) in [invalid_translation, invalid_basis].iter().enumerate() {
            let transform = light_node_transform(authored);
            assert!(
                transform.rotation.into_iter().all(f32::is_finite),
                "rotation is finite in both cases"
            );
            capture_node_transform(&mut fields, index, &transform, authored, &origin)?;
        }
        assert_eq!(
            fields
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>(),
            ["/nodes/0/rotation", "/nodes/0/scale"],
            "omit sanitized translation and guessed rotation while retaining the original finite fields"
        );
        Ok(())
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    Assembly, InteriorLightHelper, InteriorLightHelperError, InteriorLightHelperSource,
    InteriorLightPlacement, InteriorPart, ObjectContainerRef, SocTileLightAnchorError,
};
use p4kx_extraction::{SourceInventory, SourceUses, SourceValue, SourcedValue};
use p4kx_model::SocLight;
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

pub(crate) fn capture(
    assembly: &Assembly,
    items: &[Item<'_>],
    context: &PackageWriteContext<'_>,
) -> PackageResult<Option<serde_json::Value>> {
    let sources = SourceUses::with_compact_use_refs();
    let mut rows = Vec::new();
    for (node, item) in items.iter().enumerate() {
        if let Some(placed) = &item.light_placement {
            context.check()?;
            rows.push(LightPlacement::read(node, placed, assembly, &sources)?);
        }
    }
    if rows.is_empty() {
        return Ok(None);
    }
    let inventory = sources.freeze().map_err(error)?;
    serde_json::to_value(LightPlacements { rows, inventory })
        .map(Some)
        .map_err(error)
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

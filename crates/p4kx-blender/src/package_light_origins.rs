// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Assembly, InteriorLightHelper, InteriorLightHelperSource};
use p4kx_extraction::cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_extraction::{
    DataForgeOrigin, EntryOrigin, InputScope, SourceCoordinateOmission, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

use crate::PackageResult;
use crate::soc_scene_origin::SocXmlPosition;

use super::{PlacedLight, error};

#[derive(Clone, Default)]
pub(super) struct Uses {
    actual: Vec<SourceUse>,
    complete: bool,
}

impl Uses {
    const fn bound(actual: Vec<SourceUse>) -> Self {
        Self {
            complete: !actual.is_empty(),
            actual,
        }
    }

    fn together<'a>(sets: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut result = Self {
            actual: Vec::new(),
            complete: true,
        };
        for set in sets {
            result.actual.extend_from_slice(&set.actual);
            result.complete &= set.complete;
        }
        result
    }

    pub(super) fn value<T>(&self, sources: &SourceUses, value: SourceValue<T>) -> SourcedValue<T> {
        sources.value(value, &self.actual, self.complete)
    }
}

pub(super) struct Evidence {
    pub(super) state: Uses,
    pub(super) light: Uses,
    pub(super) group: Uses,
    pub(super) tile: Uses,
    pub(super) declared_helper: Uses,
    pub(super) port_name: Uses,
    pub(super) offset: Uses,
    pub(super) owner: Uses,
    pub(super) anchor: Uses,
    pub(super) effective_light: Uses,
    pub(super) helper_name: Uses,
    pub(super) helper_model: Uses,
    pub(super) helper_frame: Uses,
}

impl Evidence {
    pub(super) fn read(
        placed: &PlacedLight<'_>,
        assembly: &Assembly,
        sources: &SourceUses,
    ) -> PackageResult<Self> {
        let soc = SocUses::read(placed, sources)?;
        let (port_name, offset) = port(placed, sources)?;
        let owner = owner(placed, assembly, sources)?;
        let mut anchor = Uses::together([&soc.group, &soc.tile, &port_name, &offset, &owner]);
        // The stream selection and owner model selection need independent source proof.
        anchor.complete = false;
        let effective_light = if placed.placement.anchor.is_some() {
            Uses::together([&soc.light, &anchor])
        } else {
            soc.light.clone()
        };
        let (helper_name, helper_model) = helper(placed, assembly, sources, &soc)?;
        let helper_frame = Uses::together([&effective_light, &owner, &helper_name, &helper_model]);
        Ok(Self {
            state: soc.state,
            light: soc.light,
            group: soc.group,
            tile: soc.tile,
            declared_helper: soc.helper,
            port_name,
            offset,
            owner,
            anchor,
            effective_light,
            helper_name,
            helper_model,
            helper_frame,
        })
    }
}

#[derive(Default)]
struct SocUses {
    state: Uses,
    light: Uses,
    group: Uses,
    tile: Uses,
    helper: Uses,
}

impl SocUses {
    fn read(placed: &PlacedLight<'_>, sources: &SourceUses) -> PackageResult<Self> {
        let Some(scope) = archive_scope(sources, placed.placement.stream_origin.as_deref())? else {
            return Ok(Self::default());
        };
        let Some(group) = placed
            .light
            .group_placement
            .as_ref()
            .and_then(|group| group.source.as_ref())
        else {
            return Self::without_coordinates(&scope);
        };
        let position = Some(group.chunk);
        let element =
            |element: &SourceElementPosition, field: &str, role| -> PackageResult<SourceUse> {
                let position_xml = SocXmlPosition::element(element.clone()).map_err(error)?;
                scope
                    .at(position_xml.location(position, field.to_owned()), role)
                    .map_err(error)
            };
        let asset = element(&group.entity, "Pos/Rotate/Scale", SourceUseRole::Payload)?;
        let mut light = vec![asset.clone()];
        if let Some(baked) = &placed.light.baked_position {
            light.push(element(baked, "$element", SourceUseRole::Payload)?);
        }
        if let Some(relative) = &placed.light.relative_position {
            light.push(element(
                relative,
                "rotation/translation",
                SourceUseRole::Payload,
            )?);
        }
        let state = placed
            .light
            .component_position
            .as_ref()
            .map(|component| element(component, "$element", SourceUseRole::Selector))
            .transpose()?;
        let tile = if let Some(tile) = &group.tile {
            element(tile, "ItemPortName", SourceUseRole::Selector)?
        } else {
            element(
                &group.entity,
                "TileAttachmentParams",
                SourceUseRole::Selector,
            )?
        };
        Ok(Self {
            state: Uses::bound(state.into_iter().collect()),
            light: Uses::bound(light),
            group: Uses::bound(vec![asset]),
            tile: Uses::bound(vec![tile]),
            helper: Uses::bound(vec![element(
                &group.component,
                "helper",
                SourceUseRole::Payload,
            )?]),
        })
    }

    fn without_coordinates(scope: &SourceScope) -> PackageResult<Self> {
        let used = scope
            .at_opaque(
                SourceUseRole::Payload,
                SourceCoordinateOmission::Unavailable,
            )
            .map_err(error)?;
        let opaque = Uses {
            actual: vec![used],
            complete: false,
        };
        Ok(Self {
            state: opaque.clone(),
            light: opaque.clone(),
            group: opaque.clone(),
            tile: opaque.clone(),
            helper: opaque,
        })
    }
}

fn port(placed: &PlacedLight<'_>, sources: &SourceUses) -> PackageResult<(Uses, Uses)> {
    let Some(port) = &placed.placement.port else {
        return Ok((Uses::default(), Uses::default()));
    };
    let Some(scope) = archive_scope(sources, port.origin.as_deref())? else {
        return Ok((Uses::default(), Uses::default()));
    };
    let element = SocXmlPosition::element(port.position.clone()).map_err(error)?;
    let capture =
        |attributes: &[SourceAttributePosition], field: &str, role| -> PackageResult<Uses> {
            let mut actual = vec![
                scope
                    .at(element.location(None, "$element".to_owned()), role)
                    .map_err(error)?,
            ];
            for attribute in attributes {
                let position = SocXmlPosition::attribute(attribute).map_err(error)?;
                actual.push(
                    scope
                        .at(position.location(None, field.to_owned()), role)
                        .map_err(error)?,
                );
            }
            Ok(Uses::bound(actual))
        };
    Ok((
        capture(&port.name_positions, "name", SourceUseRole::Selector)?,
        capture(
            &port.offset_positions,
            "interactionOffset",
            SourceUseRole::Payload,
        )?,
    ))
}

fn owner(
    placed: &PlacedLight<'_>,
    assembly: &Assembly,
    sources: &SourceUses,
) -> PackageResult<Uses> {
    let mut fields = Vec::new();
    if let Some(scope) = forge_scope(assembly, sources)? {
        for field in placed.reference.source_fields() {
            fields.push(scope.at(field, SourceUseRole::Conversion).map_err(error)?);
        }
    }
    let data = Uses::bound(fields);
    let Some(node) = placed.placement.container_helper_node else {
        return Ok(data);
    };
    let model = model(assembly, sources, placed.owner, node)?;
    Ok(Uses::together([&data, &model]))
}

fn helper(
    placed: &PlacedLight<'_>,
    assembly: &Assembly,
    sources: &SourceUses,
    soc: &SocUses,
) -> PackageResult<(Uses, Uses)> {
    let InteriorLightHelper::Matched(frame) = &placed.placement.helper else {
        return Ok((Uses::default(), Uses::default()));
    };
    let name = match &frame.source {
        InteriorLightHelperSource::Group => soc.helper.clone(),
        InteriorLightHelperSource::VehiclePoint {
            entry_origin,
            bone_position,
            origin,
            ..
        } => {
            let scope = archive_scope(sources, entry_origin.as_deref())?;
            let mut actual = soc.tile.actual.clone();
            if let Some(scope) = scope {
                let used = match bone_position {
                    Some(position) => {
                        let position = SocXmlPosition::attribute(position).map_err(error)?;
                        let field = if *origin == p4kx_assembly::ValueOrigin::Elem {
                            "value"
                        } else {
                            "bone"
                        };
                        scope.at(
                            position.location(None, field.to_owned()),
                            SourceUseRole::Payload,
                        )
                    }
                    None => scope.at_opaque(
                        SourceUseRole::Payload,
                        SourceCoordinateOmission::Unavailable,
                    ),
                }
                .map_err(error)?;
                actual.push(used);
            }
            // The unique point selection also depends on the effective name and type declarations.
            Uses {
                complete: false,
                actual,
            }
        }
        _ => Uses::default(),
    };
    Ok((name, model(assembly, sources, placed.owner, frame.node)?))
}

fn model(
    assembly: &Assembly,
    sources: &SourceUses,
    owner: usize,
    node: usize,
) -> PackageResult<Uses> {
    let Some(selected) = assembly.model_node_input(owner, node) else {
        return Ok(Uses::default());
    };
    let scope = sources
        .observe(SourceInput::Archive(selected.origin()), InputScope::Payload)
        .map_err(error)?;
    let mut actual = Vec::new();
    for (node, row) in selected.nodes() {
        for (field, range) in [
            ("name", row.name()),
            ("parent", Some(row.parent())),
            ("matrix", Some(row.matrix())),
            ("scale", row.scale()),
        ] {
            if let Some(range) = range {
                actual.push(
                    scope
                        .at(
                            SourceLocation::Subresource {
                                namespace: "modelNode".to_owned(),
                                indices: vec![
                                    node as u64,
                                    row.chunk() as u64,
                                    range.start,
                                    range.end,
                                ],
                                field: field.to_owned(),
                            },
                            SourceUseRole::Conversion,
                        )
                        .map_err(error)?,
                );
            }
        }
    }
    if let Some((scope, location)) =
        forge_scope(assembly, sources)?.zip(selected.geometry_selector())
    {
        actual.push(scope.at(location, SourceUseRole::Selector).map_err(error)?);
    }
    if let Some(scope) = archive_scope(sources, selected.definition_origin())? {
        for location in selected.cdf_model_selectors() {
            actual.push(scope.at(location, SourceUseRole::Selector).map_err(error)?);
        }
    }
    // Node and model selection can depend on other consulted archive entries.
    Ok(Uses {
        actual,
        complete: false,
    })
}

fn forge_scope(assembly: &Assembly, sources: &SourceUses) -> PackageResult<Option<SourceScope>> {
    let input = match assembly.forge_origin() {
        Some(DataForgeOrigin::Archive(origin)) => Some(SourceInput::Archive(origin)),
        Some(DataForgeOrigin::File(origin)) => Some(SourceInput::File(origin)),
        _ => None,
    };
    input
        .map(|input| sources.observe(input, InputScope::Payload))
        .transpose()
        .map_err(error)
}

fn archive_scope(
    sources: &SourceUses,
    origin: Option<&EntryOrigin>,
) -> PackageResult<Option<SourceScope>> {
    origin
        .map(|origin| sources.observe(SourceInput::Archive(origin), InputScope::Payload))
        .transpose()
        .map_err(error)
}

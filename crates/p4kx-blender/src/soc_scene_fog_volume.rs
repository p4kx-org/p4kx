// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue, SourcedValue};
use p4kx_model::SocEntityComponent;
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::children;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin, matching_attributes, read_attribute};
use crate::soc_scene_rooms::position;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FogVolume {
    volume_type: SourcedValue<u32>,
    global_density: SourcedValue<f32>,
    soft_edges: SourcedValue<f32>,
    ramp_start: SourcedValue<f32>,
    ramp_end: SourcedValue<f32>,
    ramp_influence: SourcedValue<f32>,
    affects_this_area_only: SourcedValue<bool>,
    max_distance: SourcedValue<f32>,
    sizes: Vec<FogVolumeSize>,
}

impl FogVolume {
    pub(crate) fn read(
        component: SocEntityComponent<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = component.element();
        let owner = component
            .properties_element()
            .ok_or(SocSceneError::Position)?;
        let [component_name, component_type] = shape_selectors(element, origin)?;
        let selectors = [
            origin.node(owner, SourceUseRole::Selector)?,
            component_name,
            component_type,
        ];
        let single =
            |name| origin.parsed_attribute_with_selectors(element, name, finite_single, &selectors);
        Ok(Self {
            volume_type: origin.parsed_attribute_with_selectors(
                element,
                "volumeType",
                |value| value.parse().ok(),
                &selectors,
            )?,
            global_density: single("globalDensity")?,
            soft_edges: single("softEdges")?,
            ramp_start: single("rampStart")?,
            ramp_end: single("rampEnd")?,
            ramp_influence: single("rampInfluence")?,
            affects_this_area_only: origin.parsed_attribute_with_selectors(
                element,
                "affectsThisAreaOnly",
                boolean,
                &selectors,
            )?,
            max_distance: single("maxDistance")?,
            sizes: children(element, "size")
                .filter(|size| {
                    read_attribute(*size, "__type", Some) == SourceValue::Present("Vec3")
                })
                .map(|size| FogVolumeSize::read(size, origin, &selectors))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct FogVolumeSize {
    position: SocXmlPosition,
    x: SourcedValue<f32>,
    y: SourcedValue<f32>,
    z: SourcedValue<f32>,
}

impl FogVolumeSize {
    fn read(
        element: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        component_selectors: &[SourceUse],
    ) -> Result<Self, SocSceneError> {
        let mut selectors = component_selectors.to_vec();
        selectors.extend(shape_selectors(element, origin)?);
        let single =
            |name| origin.parsed_attribute_with_selectors(element, name, finite_single, &selectors);
        Ok(Self {
            position: position(element)?,
            x: single("x")?,
            y: single("y")?,
            z: single("z")?,
        })
    }
}

fn shape_selectors(
    element: SourceElement<'_>,
    origin: &XmlOrigin<'_>,
) -> Result<[SourceUse; 2], SocSceneError> {
    let declared_type = matching_attributes(element, "__type")
        .next()
        .ok_or(SocSceneError::Position)?;
    let role = SourceUseRole::Selector;
    Ok([
        origin.node(element, role)?,
        origin.attribute(&declared_type, role)?.1,
    ])
}

fn finite_single(value: &str) -> Option<f32> {
    let value: f32 = value.parse().ok()?;
    value.is_finite().then_some(value)
}

fn boolean(value: &str) -> Option<bool> {
    if value == "1" || value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value == "0" || value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

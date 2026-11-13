// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use p4kx_extraction::cryxml::SourceElement;
use p4kx_model::{SocBakedLightSource, SocLightGroupSource, SocLightMode, SocLightSource};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Light {
    position: SocXmlPosition,
    #[serde(rename = "lightType")]
    kind: SourcedValue<String>,
    use_temperature: SourcedValue<bool>,
    modes: Vec<Mode>,
}

impl Light {
    pub(crate) fn read(
        light: SocLightSource<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = light.element();
        Ok(Self {
            position: position(element)?,
            kind: origin
                .named(element, "lightType", light.light_type())?
                .map(str::to_owned),
            use_temperature: origin.named(element, "useTemperature", light.use_temperature())?,
            modes: light
                .modes()
                .map(|mode| Mode::read(mode, origin))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Mode {
    position: SocXmlPosition,
    kind: ModeKind,
    intensity: SourcedValue<f32>,
    temperature: SourcedValue<f32>,
    light_style: SourcedValue<u32>,
    colours: Vec<SocXmlPosition>,
}

impl Mode {
    fn read(mode: SocLightMode<'_>, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        let element = mode.element();
        Ok(Self {
            position: position(element)?,
            kind: ModeKind::from(mode.kind()),
            intensity: origin.named(element, "intensity", mode.intensity())?,
            temperature: origin.named(element, "temperature", mode.temperature())?,
            light_style: origin.named(element, "lightStyle", mode.light_style())?,
            colours: positions(mode.colours())?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum ModeKind {
    Default,
    Auxiliary,
    Emergency,
    Cinematic,
    Other,
}

impl From<p4kx_model::LightState> for ModeKind {
    fn from(kind: p4kx_model::LightState) -> Self {
        use p4kx_model::LightState;
        match kind {
            LightState::Default => Self::Default,
            LightState::Auxiliary => Self::Auxiliary,
            LightState::Emergency => Self::Emergency,
            LightState::Cinematic => Self::Cinematic,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightGroup {
    position: SocXmlPosition,
    baked_sections: Vec<SocXmlPosition>,
    lights: Vec<BakedLight>,
}

impl LightGroup {
    pub(crate) fn read(
        group: SocLightGroupSource<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        Ok(Self {
            position: position(group.element())?,
            baked_sections: positions(group.baked_sections())?,
            lights: group
                .lights()
                .map(|light| BakedLight::read(light, origin))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct BakedLight {
    section: SocXmlPosition,
    position: SocXmlPosition,
    relative_frames: Vec<SocXmlPosition>,
    components: Vec<Light>,
}

impl BakedLight {
    fn read(light: SocBakedLightSource<'_>, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        Ok(Self {
            section: position(light.section())?,
            position: position(light.element())?,
            relative_frames: positions(light.relative_frames())?,
            components: light
                .components()
                .map(|component| Light::read(component, origin))
                .collect::<Result<_, _>>()?,
        })
    }
}

fn positions<'document>(
    elements: impl Iterator<Item = SourceElement<'document>>,
) -> Result<Vec<SocXmlPosition>, SocSceneError> {
    elements.map(position).collect()
}

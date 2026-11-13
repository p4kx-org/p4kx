// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Transform;
use crate::mtl_xml::XmlElement;
use crate::soc_entities::{child, number};

/// The state of a light whose values a [`SocLight`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LightState {
    /// The default state, `defaultState`.
    Default,
    /// The auxiliary power state, `auxiliaryState`.
    Auxiliary,
    /// The emergency state, `emergencyState`.
    Emergency,
    /// The cinematic state, `cinematicState`.
    Cinematic,
    /// The off state, `offState`.
    Off,
}

impl LightState {
    const ORDER: [(Self, &'static str); 4] = [
        (Self::Default, "defaultState"),
        (Self::Auxiliary, "auxiliaryState"),
        (Self::Emergency, "emergencyState"),
        (Self::Cinematic, "cinematicState"),
    ];

    pub(crate) fn from_element(element: p4kx_cryxml::SourceElement<'_>) -> Option<Self> {
        if element.namespace().is_some() {
            return None;
        }
        Self::ORDER
            .into_iter()
            .chain([(Self::Off, "offState")])
            .find_map(|(kind, name)| (element.name() == name).then_some(kind))
    }

    /// Returns the stable code of the state, e.g. `default`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Auxiliary => "auxiliary",
            Self::Emergency => "emergency",
            Self::Cinematic => "cinematic",
            Self::Off => "off",
        }
    }
}

/// A light a `.soc` file places: the `EntityComponentLight` of a `Light` entity, or of a light of
/// a `LightGroup`'s `BakedInLights`.
///
/// The values are those of the first state, in the order default, auxiliary, emergency,
/// cinematic, whose intensity is above zero, or of the default state when none is.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SocLight {
    /// The light's name; for a light of a group the name of the light.
    pub name: String,
    /// The name of the `LightGroup` entity the light belongs to.
    pub group: Option<String>,
    /// The transform from the light's space to the container's, in `CryEngine` coordinates, Z up
    /// and in metres; the light shines along its +X axis.
    pub transform: Transform,
    /// The `lightType` as the file spells it, e.g. `Omni`, `SoftOmni`, `Projector`, `Directional`,
    /// `Ambient` or `Planar`.
    pub light_type: String,
    /// The state the values are those of.
    pub state: LightState,
    /// The colour of the state as stored, linear red, green and blue in 0 to 1.
    pub colour: [f32; 3],
    /// The intensity of the state as stored, in the engine's own unit.
    pub intensity: f32,
    /// The colour temperature of the state in kelvin.
    pub temperature: f32,
    /// Whether the colour temperature tints the light (`useTemperature`).
    pub use_temperature: bool,
    /// The distance beyond which the light has no effect (`lightRadius`), in metres; zero when
    /// the file gives none.
    pub radius: f32,
    /// The full opening angle of a projector in degrees (`FOV`); zero when the file gives none.
    pub fov: f32,
    /// The `focusedBeam` value of the light; every light type carries it and it is not used.
    pub focused_beam: f32,
}

struct StateValues {
    state: LightState,
    colour: [f32; 3],
    intensity: f32,
    temperature: f32,
}

fn state_values<'a>(component: impl XmlElement<'a>) -> Option<StateValues> {
    let mut chosen: Option<StateValues> = None;
    for (state, tag) in LightState::ORDER {
        let Some(element) = child(component, tag) else {
            continue;
        };
        let intensity = number(element, "intensity").unwrap_or(0.0).max(0.0);
        let colour = child(element, "color").map_or([1.0; 3], |color| {
            ["r", "g", "b"].map(|channel| number(color, channel).unwrap_or(1.0).clamp(0.0, 1.0))
        });
        let values = StateValues {
            state,
            colour,
            intensity,
            temperature: number(element, "temperature").unwrap_or(6500.0),
        };
        let on = values.intensity > 0.0;
        if chosen.is_none() || on {
            chosen = Some(values);
        }
        if on {
            break;
        }
    }
    chosen
}

pub(crate) fn read_light<'a>(
    name: &str,
    group: Option<&str>,
    transform: Transform,
    component: impl XmlElement<'a>,
) -> Option<SocLight> {
    let light_type = component.attribute("lightType")?.trim().to_owned();
    let values = state_values(component)?;
    let size = child(component, "sizeParams");
    let projector = child(component, "projectorParams");
    Some(SocLight {
        name: name.to_owned(),
        group: group.map(str::to_owned),
        transform,
        light_type,
        state: values.state,
        colour: values.colour,
        intensity: values.intensity,
        temperature: values.temperature,
        use_temperature: component
            .attribute("useTemperature")
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
        radius: size
            .and_then(|size| number(size, "lightRadius"))
            .unwrap_or(0.0)
            .max(0.0),
        fov: projector
            .and_then(|projector| number(projector, "FOV"))
            .unwrap_or(0.0)
            .max(0.0),
        focused_beam: projector
            .and_then(|projector| number(projector, "focusedBeam"))
            .unwrap_or(0.0),
    })
}

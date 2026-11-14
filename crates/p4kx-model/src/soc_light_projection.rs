// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_xml::XmlElement;
use crate::soc_entities::{MAX_SOC_PLACED, SocEntities, SocEntitiesError, child, numbers};
use crate::soc_light::read_light;
use crate::{SocGroupPlacement, Transform};

pub(crate) fn read_lights<'a>(
    entity: impl XmlElement<'a>,
    (name, class): (&str, &str),
    transform: Option<Transform>,
    chunk: Option<usize>,
    out: &mut SocEntities,
) -> Result<(), SocEntitiesError> {
    if class.eq_ignore_ascii_case("Light") {
        if let Some(component) =
            child(entity, "PropertiesDataCore").and_then(|data| child(data, "EntityComponentLight"))
        {
            push_light(out, name, None, transform, component, None)?;
        }
    } else if let Some(group) = child(entity, "EntityComponentLightGroup")
        && let Some(baked) = child(group, "BakedInLights")
    {
        let group_placement =
            transform.map(|frame| SocGroupPlacement::read(entity, group, frame, chunk));
        for light in baked.children() {
            if !light.name().eq_ignore_ascii_case("Light") {
                continue;
            }
            let Some(component) = child(light, "EntityComponentLight") else {
                continue;
            };
            let relative = child(light, "RelativeXForm");
            let own = light.attribute("Name").unwrap_or(name);
            let before = out.lights.len();
            push_light(out, own, Some(name), transform, component, relative)?;
            if let Some(placed) = out.lights.get_mut(before) {
                placed.group_placement.clone_from(&group_placement);
                if chunk.is_some() {
                    placed.baked_position = Some(light.position());
                    placed.component_position = Some(component.position());
                    placed.relative_position = relative.map(XmlElement::position);
                }
            }
        }
    }
    Ok(())
}

fn push_light<'a, E: XmlElement<'a>>(
    out: &mut SocEntities,
    name: &str,
    group: Option<&str>,
    entity: Option<Transform>,
    component: E,
    relative: Option<E>,
) -> Result<(), SocEntitiesError> {
    if out.lights.len() >= MAX_SOC_PLACED {
        return Err(SocEntitiesError::TooMany {
            what: "lights",
            limit: MAX_SOC_PLACED,
        });
    }
    let placed = match (entity, relative) {
        (Some(entity), None) => Some(entity),
        (Some(entity), Some(relative)) => {
            relative_transform(relative).and_then(|r| entity.then(&r).finite())
        }
        (None, _) => None,
    };
    match placed.and_then(|transform| read_light(name, group, transform, component)) {
        Some(light) => out.lights.push(light),
        None => out.invalid += 1,
    }
    Ok(())
}

fn relative_transform<'a>(element: impl XmlElement<'a>) -> Option<Transform> {
    let rotation = numbers::<4>(element.attribute("rotation"))?;
    let translation = numbers::<3>(element.attribute("translation"))?;
    Transform::from_quaternion(rotation, translation)
}

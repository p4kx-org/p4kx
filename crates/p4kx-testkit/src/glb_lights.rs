// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use gltf::khr_lights_punctual::Kind;

use crate::glb::CheckedGlb;

/// Checks the `KHR_lights_punctual` rules a writer can break: the extension is declared when
/// lights exist, colours lie in 0 to 1, intensities are finite and not negative, ranges are
/// positive, spot cone angles satisfy 0 <= inner < outer <= pi/2, and every light is used.
pub(crate) fn check_lights(glb: &CheckedGlb) -> Result<(), String> {
    let lights: Vec<_> = glb.document.lights().map_or_default(Iterator::collect);
    let used = glb
        .document
        .extensions_used()
        .any(|name| name == "KHR_lights_punctual");
    if lights.is_empty() == used {
        return Err("KHR_lights_punctual is declared exactly when lights exist".to_owned());
    }
    let mut referenced = vec![false; lights.len()];
    for node in glb.document.nodes() {
        if let Some(light) = node.light()
            && let Some(slot) = referenced.get_mut(light.index())
        {
            *slot = true;
        }
    }
    for light in &lights {
        let index = light.index();
        if !referenced.get(index).copied().unwrap_or(false) {
            return Err(format!("light {index} is used by no node"));
        }
        if light.color().iter().any(|c| !(0.0..=1.0).contains(c)) {
            return Err(format!("light {index} has a colour outside 0 to 1"));
        }
        if !light.intensity().is_finite() || light.intensity() < 0.0 {
            return Err(format!("light {index} has an invalid intensity"));
        }
        if light
            .range()
            .is_some_and(|range| !(range.is_finite() && range > 0.0))
        {
            return Err(format!("light {index} has a range that is not positive"));
        }
        if let Kind::Spot {
            inner_cone_angle,
            outer_cone_angle,
        } = light.kind()
        {
            let valid = inner_cone_angle >= 0.0
                && inner_cone_angle < outer_cone_angle
                && outer_cone_angle <= std::f32::consts::FRAC_PI_2;
            if !valid {
                return Err(format!("light {index} has invalid cone angles"));
            }
        }
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::Material;

use crate::surface_finish::SurfaceFinish;
use crate::texture_pixels::Tints;
use crate::texture_plan::{Job, Key};
use crate::texture_roles::{Source, Sources};

pub(crate) fn own_packed(
    material: &Material,
    found: &Sources,
    explicit_gloss: Vec<Source>,
    specular: Option<(String, [u8; 3])>,
    tints: Tints,
) -> Option<Job> {
    let gloss = found.normal.as_ref().filter(|normal| normal.gloss);
    (gloss.is_some() || !explicit_gloss.is_empty() || specular.is_some()).then(|| Job {
        key: Key::MetallicRoughness {
            gloss: gloss.map(Source::identity),
            explicit_gloss: explicit_gloss.iter().map(Source::identity).collect(),
            specular,
            finish: SurfaceFinish::material(material),
        },
        diffuse: None,
        normal: gloss.cloned(),
        specular: found.specular.clone(),
        explicit_gloss,
        tints,
        layered: None,
    })
}

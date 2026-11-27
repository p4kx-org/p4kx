// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::Material;

use crate::factory_bow_reflectance;
use crate::layer_jobs::layer_jobs;
use crate::material_alpha::uses_alpha;
use crate::shading::shading;
use crate::texture_pixels::Tints;
use crate::texture_report::TextureRole;
use crate::texture_roles::{
    Source, Sources, draws_by_opacity_map, explicit_gloss_sources, sources_with,
};

use super::{Failure, Job, Key, Wanted};

const NEUTRAL: [u8; 3] = [u8::MAX; 3];
const REFUSED: &str = "the path is not a safe texture path";

pub(super) struct Jobs {
    pub(super) images: Wanted,
    pub(super) specular_colour: Option<usize>,
    pub(super) fallback: Option<usize>,
    pub(super) refused: Vec<Failure>,
}

fn quantise(colour: [f32; 3]) -> [u8; 3] {
    colour.map(|channel| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the channel is between 0 and 1"
        )]
        let byte = (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        byte
    })
}

fn refusals(refused: &[(TextureRole, String)]) -> Vec<Failure> {
    refused
        .iter()
        .map(|(role, file)| Failure {
            role: *role,
            file: file.clone(),
            reason: REFUSED.to_owned(),
        })
        .collect()
}

fn tints_of(material: &Material) -> Tints {
    let shade = shading(material);
    Tints {
        diffuse: shade.diffuse.map(|channel| channel.clamp(0.0, 1.0)),
        specular: shade.specular.map(|channel| channel.clamp(0.0, 1.0)),
        decal: material
            .decal_colours
            .map(|colours| colours.map(|colour| colour.map(|channel| channel.clamp(0.0, 1.0)))),
    }
}

/// Returns the job of the base colour image of the diffuse map `diffuse` of `material`, with the
/// specular map of `found`, if any.
fn base_job(
    material: &Material,
    (found, diffuse): (&Sources, &Source),
    specular: Option<(String, [u8; 3])>,
    tints: Tints,
) -> Job {
    let tint = if specular.is_some() {
        quantise(tints.diffuse)
    } else {
        NEUTRAL
    };
    Job {
        key: Key::Base {
            diffuse: diffuse.identity(),
            specular,
            tint,
            decal: tints
                .decal
                .filter(|_| diffuse.opacity)
                .map(|colours| colours.map(quantise)),
            keep_alpha: diffuse.opacity || uses_alpha(material),
            opacity_source: diffuse.opacity,
        },
        diffuse: Some(diffuse.clone()),
        normal: None,
        specular: found.specular.clone(),
        explicit_gloss: Vec::new(),
        tints,
        layered: None,
    }
}

/// Registers the jobs for the images of `material` with `add`, which returns a job's number, and
/// returns the jobs with the textures refused for their unsafe paths.
pub(super) fn jobs_of(
    material: &Material,
    approximate_factory_bow: bool,
    add: &mut impl FnMut(Job) -> usize,
) -> Jobs {
    let mut found = sources_with(material, draws_by_opacity_map(material));
    if found.diffuse.as_ref().is_none_or(|diffuse| diffuse.opacity) {
        found.specular = None;
    }
    let explicit_gloss = explicit_gloss_sources(material, &mut found.refused);
    let mut refused = refusals(&found.refused);
    let tints = tints_of(material);
    let specular_key = found
        .specular
        .as_ref()
        .map(|specular| (specular.identity(), quantise(tints.specular)));
    let (layer_base, layer_normal, layer_finish, layer_refused) =
        layer_jobs(material, &found, tints);
    refused.extend(refusals(&layer_refused));
    let base = match (&found.diffuse, layer_base) {
        (Some(diffuse), _) => Some(add(base_job(
            material,
            (&found, diffuse),
            specular_key.clone(),
            tints,
        ))),
        (None, layered) => layered.map(&mut *add),
    };
    let plain_normal = found.normal.as_ref().map(|normal| {
        add(Job {
            key: Key::Normal {
                normal: normal.identity(),
            },
            diffuse: None,
            normal: found.normal.clone(),
            specular: None,
            explicit_gloss: Vec::new(),
            tints,
            layered: None,
        })
    });
    let (normal, fallback) = layer_normal.map_or((plain_normal, None), |layered| {
        (Some(add(layered)), plain_normal)
    });
    let packed = layer_finish.map(&mut *add).or_else(|| {
        crate::texture_finish_plan::own_packed(
            material,
            &found,
            explicit_gloss,
            specular_key,
            tints,
        )
        .map(&mut *add)
    });
    let specular_colour = approximate_factory_bow
        .then(|| factory_bow_reflectance::job(material, tints))
        .flatten()
        .map(add);
    Jobs {
        images: [base, normal, packed],
        specular_colour,
        fallback,
        refused,
    }
}

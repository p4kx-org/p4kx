// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_texture::RgbaImage;

use super::{Budget, Outcome, Squares, TINTS, normal_jobs, run, run_guarded, source};
use crate::layer_finish;
use crate::layer_make;
use crate::layer_plan::{LayerPlan, Layered};
use crate::surface_finish::SurfaceFinish;
use crate::texture_cache::{Decoded, MAX_DECODE_PIXELS, SourceStore};
use crate::texture_loader::{LoadedTexture, TextureLoadError, TextureLoader};
use crate::texture_origins::{TextureComponents, TextureReadId, TextureReadUse};
use crate::texture_plan::{Job, Key};

struct Odd {
    layer_side: u32,
}

impl TextureLoader for Odd {
    fn load(&self, path: &str, _mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        if path.contains("blend") {
            return Err(TextureLoadError::NotFound);
        }
        let side = self.layer_side;
        RgbaImage::new(side, side, vec![200; (side * side * 4) as usize])
            .map(|image| LoadedTexture::new(image, false))
            .map_err(|error| TextureLoadError::Unreadable(error.to_string()))
    }

    fn dimensions(&self, path: &str, _mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        if path.contains("blend") {
            return Err(TextureLoadError::NotFound);
        }
        Ok(Some((self.layer_side, self.layer_side)))
    }
}

struct Picky;

impl TextureLoader for Picky {
    fn load(&self, path: &str, _mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        let side = if path.contains("bad") { 0 } else { 4 };
        RgbaImage::new(side, side, vec![200; (side * side * 4) as usize])
            .map(|image| LoadedTexture::new(image, true))
            .map_err(|error| TextureLoadError::Unreadable(error.to_string()))
    }

    fn dimensions(&self, path: &str, _mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        let side = if path.contains("bad") { 0 } else { 4 };
        Ok(Some((side, side)))
    }
}

fn layered_job(with_mask: bool) -> Job {
    layered_job_of("l", with_mask)
}

fn layered_job_of(prefix: &str, with_mask: bool) -> Job {
    let layer = |at: usize, share: f32| LayerPlan {
        gloss: Vec::new(),
        finish: None,
        roughness: 0.5,
        at,
        share,
        diffuse: Some((
            source(format!("objects/{prefix}{at}_diff.dds"), false),
            [1.0; 2],
        )),
        normal: None,
        specular_map: None,
        colour: [1.0; 3],
        specular: [0.0; 3],
        metal: 0.0,
    };
    let layered = Layered {
        layers: vec![layer(0, 1.0), layer(1, 0.0)],
        mask: with_mask.then(|| source("objects/m_blend.dds".to_owned(), false)),
        refused: Vec::new(),
    };
    Job {
        key: Key::LayerBase {
            blend: layered.fingerprint(false),
        },
        diffuse: None,
        normal: None,
        specular: None,
        explicit_gloss: Vec::new(),
        tints: TINTS,
        layered: Some(layered),
    }
}

#[test]
fn the_image_count_has_its_own_reason() {
    let outcomes = run(
        &normal_jobs(3),
        0,
        &Squares::new(2),
        Budget {
            images: 1,
            pixels: u64::MAX,
        },
    );

    let reasons: Vec<&str> = outcomes
        .iter()
        .flat_map(|outcome| &outcome.failures)
        .map(|failure| failure.reason.as_str())
        .collect();
    assert_eq!(reasons.len(), 2);
    assert!(
        reasons.iter().all(
            |reason| reason.contains("as many as allowed") && !reason.contains("--texture-mip")
        ),
        "{reasons:?}"
    );
}

#[test]
fn a_layered_job_whose_textures_decode_more_than_the_limit_is_refused_before_decoding() {
    let loader = Squares::new(12_000).sized();

    let outcomes = run(&[layered_job(true)], 0, &loader, Budget::DEFAULT);

    assert_eq!(loader.loads(), 0, "nothing decoded");
    let failure = outcomes
        .first()
        .and_then(|outcome| outcome.failures.first())
        .map(|failure| (failure.reason.clone(), failure.file.clone()));
    let (reason, file) = failure.unwrap_or_default();
    assert!(reason.contains("2^27"), "{reason}");
    assert!(
        file.contains("layered base colour image of 2 layers"),
        "{file}"
    );
}

#[test]
fn a_missing_blend_map_leaves_the_shares_by_position_and_is_reported_once() {
    let loader = Odd { layer_side: 4 };

    let outcomes = run(&[layered_job(true)], 0, &loader, Budget::DEFAULT);

    let outcome = outcomes.first();
    let size = outcome
        .and_then(|outcome| outcome.image.as_ref())
        .map(|image| (image.width, image.height));
    assert_eq!(
        size,
        Some((4, 4)),
        "sized by the first map that can be read"
    );
    let failures = outcome.map_or(0, |outcome| outcome.failures.len());
    assert_eq!(failures, 1, "the blend map once");
}

#[test]
fn a_fallback_whose_layered_image_was_made_takes_no_slot_a_needed_fallback_gets() {
    let mut jobs = vec![layered_job_of("good", false), layered_job_of("bad", false)];
    jobs.extend(normal_jobs(2));
    let budget = Budget {
        images: 2,
        pixels: u64::MAX,
    };

    let unguarded = run(&jobs, 0, &Picky, budget);
    let guarded = run_guarded(
        &jobs,
        0,
        &Picky,
        budget,
        &[vec![], vec![], vec![0], vec![1]],
    );

    let made = |outcomes: &[Outcome]| -> Vec<bool> {
        outcomes
            .iter()
            .map(|outcome| outcome.image.is_some())
            .collect()
    };
    assert_eq!(made(&unguarded), [true, false, false, false]);
    assert_eq!(made(&guarded), [true, false, false, true]);
    assert!(guarded.iter().all(|outcome| {
        outcome
            .failures
            .iter()
            .all(|f| !f.reason.contains("as many as allowed"))
    }));
}

#[test]
fn a_map_without_pixels_counts_as_not_loaded() {
    let loader = Odd { layer_side: 0 };

    let outcomes = run(&[layered_job(false)], 0, &loader, Budget::DEFAULT);

    assert_eq!(outcomes.len(), 1);
    assert!(outcomes.iter().all(|outcome| outcome.image.is_none()));
}

fn provenance_image(
    id: usize,
    rgba: [u8; 4],
) -> Result<Arc<LoadedTexture>, p4kx_texture::TextureError> {
    let image = RgbaImage::new(1, 1, rgba.to_vec())?;
    Ok(Arc::new(
        LoadedTexture::new(image, true).with_read(TextureReadId::new(id)),
    ))
}

fn has_colour_read(uses: &[TextureReadUse], id: usize) -> bool {
    uses.iter().any(|read| {
        read.read == Some(TextureReadId::new(id)) && read.components == TextureComponents::Color
    })
}

#[test]
fn layered_outputs_retain_sampled_specular_and_exclude_fully_masked_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let mut job = layered_job(true);
    let layered = job.layered.as_mut().ok_or("the layered job")?;
    for layer in &mut layered.layers {
        let file = format!("objects/l{}_spec.dds", layer.at);
        layer.specular_map = Some((source(file, false), [1.0; 2]));
        layer.specular = [1.0; 3];
        layer.metal = 1.0;
    }
    let decoded = provenance_sources()?;
    let mut base = Outcome::default();
    let mut packed = Outcome::default();
    let mut failures = Vec::new();
    layer_make::make_base(&job, &decoded, &mut base, &mut failures).ok_or("base image")??;
    layer_finish::make(&job, &decoded, &mut packed, &mut failures).ok_or("packed image")??;
    assert!(failures.is_empty(), "both selected source paths decode");
    for outcome in [&base, &packed] {
        assert!(
            has_colour_read(&outcome.texture_sources, 4),
            "the visible layer supplies specular RGB"
        );
        assert!(
            !has_colour_read(&outcome.texture_sources, 5),
            "the masked layer supplies no specular RGB"
        );
        assert_eq!(
            outcome.material_layers,
            [0],
            "the selected original layer is preserved"
        );
    }
    assert!(
        has_colour_read(&base.texture_sources, 2),
        "base diffuse is still sampled"
    );
    assert!(
        packed.metallic && !packed.roughness,
        "the packed job enables the metallic channel alone"
    );
    Ok(())
}

fn provenance_sources() -> Result<Decoded, p4kx_texture::TextureError> {
    [
        ("objects/m_blend.dds", 1, [255, 0, 0, 255]),
        ("objects/l0_diff.dds", 2, [160, 100, 33, 255]),
        ("objects/l1_diff.dds", 3, [99, 101, 205, 255]),
        ("objects/l0_spec.dds", 4, [102, 255, 135, 108]),
        ("objects/l1_spec.dds", 5, [25, 240, 81, 78]),
    ]
    .into_iter()
    .map(|(path, id, pixel)| Ok((path.to_owned(), Ok(provenance_image(id, pixel)?))))
    .collect()
}

struct LayerBudgetHeaders;

impl TextureLoader for LayerBudgetHeaders {
    fn dimensions(&self, path: &str, _: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        let dimensions = if matches!(
            path,
            "objects/large_diff.dds" | "objects/large_ddna.dds" | "objects/large_spec.dds"
        ) {
            let width = u32::try_from(MAX_DECODE_PIXELS - 1)
                .map_err(|error| TextureLoadError::Unreadable(error.to_string()))?;
            (width, 1)
        } else {
            (2, 2)
        };
        Ok(Some(dimensions))
    }

    fn load(&self, _: &str, _: u32) -> Result<LoadedTexture, TextureLoadError> {
        Err(TextureLoadError::Unreadable(
            "header-only fixture".to_owned(),
        ))
    }
}

#[test]
fn an_oversized_finish_dependency_demotes_specular_for_both_jobs()
-> Result<(), Box<dyn std::error::Error>> {
    let mut base = layered_job(false);
    let layered = base.layered.as_mut().ok_or("a layered base job")?;
    let owner = layered.layers.first_mut().ok_or("a first layer")?;
    owner.specular_map = Some((source("objects/l0_spec.dds".to_owned(), false), [1.0; 2]));
    let no_gloss_base = layered.fingerprint(false);
    let owner = layered.layers.first_mut().ok_or("a first layer")?;
    owner.normal = Some((source("objects/large_ddna.dds".to_owned(), true), [1.0; 2]));
    owner.finish = Some(SurfaceFinish::new(0.5, Some(0.8)));
    let with_gloss_base = layered.fingerprint(false);
    let pack_key = layered.finish_fingerprint();
    assert_ne!(
        no_gloss_base, with_gloss_base,
        "the base identity includes its finish budget dependencies"
    );
    base.key = Key::LayerBase {
        blend: with_gloss_base,
    };
    let pack = Job {
        key: Key::LayerFinish { blend: pack_key },
        ..base.clone()
    };
    let mut store = SourceStore::new(&LayerBudgetHeaders, 0, 0);
    for original in [&base, &pack] {
        let prepared = store.admit_layer_specular(original);
        let expected = prepared
            .job()
            .layered
            .as_ref()
            .ok_or("retained mandatory inputs")?;
        assert!(
            expected.has_sampled_finish() && !expected.has_specular_map(),
            "only the optional map exceeds the finish decode budget"
        );
        assert_eq!(
            prepared.job().key,
            original.key,
            "original job identity is retained"
        );
        let [refusal] = prepared.failures.as_slice() else {
            return Err("one optional source failed the combined admission".into());
        };
        assert!(
            refusal.reason.contains("exceed the texture decode budget"),
            "the original reason is explicit"
        );
    }
    Ok(())
}

mod case_budget;

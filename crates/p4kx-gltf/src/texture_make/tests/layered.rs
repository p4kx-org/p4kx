// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::RgbaImage;

use super::{Budget, Outcome, Squares, TINTS, normal_jobs, run, run_guarded, source};
use crate::layer_plan::{LayerPlan, Layered};
use crate::texture_loader::{LoadedTexture, TextureLoadError, TextureLoader};
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
        at,
        share,
        diffuse: Some((
            source(format!("objects/{prefix}{at}_diff.dds"), false),
            [1.0; 2],
        )),
        normal: None,
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

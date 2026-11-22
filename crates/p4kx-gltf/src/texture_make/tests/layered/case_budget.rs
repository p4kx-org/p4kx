// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use super::{LayerBudgetHeaders, layered_job, source};
use crate::layer_plan::LayerMap;
use crate::surface_finish::SurfaceFinish;
use crate::texture_cache::SourceStore;
use crate::texture_plan::{Job, Key};

#[test]
fn case_distinct_inputs_with_different_decode_budgets_cannot_share_a_layered_image()
-> Result<(), Box<dyn Error>> {
    for map in [LayerMap::Diffuse, LayerMap::Normal, LayerMap::Specular] {
        let lower = case_job(map, false)?;
        let upper = case_job(map, true)?;
        let mut store = SourceStore::new(&LayerBudgetHeaders, 0, 0);
        for (large, small) in lower.iter().zip(&upper) {
            assert_ne!(
                large.key, small.key,
                "exact base, finish and optional paths participate in both keys"
            );
            let refused = store.admit_layer_specular(large);
            let admitted = store.admit_layer_specular(small);
            let [failure] = refused.failures.as_slice() else {
                return Err("exact lowercase source exhausts the available budget".into());
            };
            assert!(
                failure.reason.contains("exceed the texture decode budget"),
                "the different header sizes reach the paired admission"
            );
            assert!(
                admitted.failures.is_empty(),
                "the exact uppercase source fits"
            );
            for (prepared, selected) in [(refused, false), (admitted, true)] {
                let layered = prepared
                    .job()
                    .layered
                    .as_ref()
                    .ok_or("prepared layer inputs")?;
                assert_eq!(
                    layered.has_specular_map(),
                    selected,
                    "base and finish retain the same selected specular source"
                );
            }
        }
    }
    Ok(())
}

fn case_job(map: LayerMap, small: bool) -> Result<[Job; 2], Box<dyn Error>> {
    let mut job = layered_job(false);
    let layered = job.layered.as_mut().ok_or("a resolved layered material")?;
    layered.layers.truncate(1);
    let layer = layered
        .layers
        .first_mut()
        .ok_or("the selected original layer")?;
    layer.specular_map = Some((source("objects/l0_spec.dds".to_owned(), false), [1.0; 2]));
    let (slot, filename) = match map {
        LayerMap::Diffuse => (&mut layer.diffuse, "objects/large_diff.dds"),
        LayerMap::Normal => (&mut layer.normal, "objects/large_ddna.dds"),
        LayerMap::Specular => (&mut layer.specular_map, "objects/large_spec.dds"),
    };
    let filename = if small {
        filename.replace("large_", "LARGE_")
    } else {
        filename.to_owned()
    };
    *slot = Some((source(filename, map == LayerMap::Normal), [1.0; 2]));
    layer.finish = Some(SurfaceFinish::new(0.5, Some(0.8)));
    let finish = Key::LayerFinish {
        blend: layered.finish_fingerprint(),
    };
    job.key = Key::LayerBase {
        blend: layered.fingerprint(false),
    };
    let packed = Job {
        key: finish,
        ..job.clone()
    };
    Ok([job, packed])
}

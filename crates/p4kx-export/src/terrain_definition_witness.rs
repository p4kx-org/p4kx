// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;
use std::io::Read as _;

use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use super::{ecosystem, geology_set, geology_texture_set, shading_class, soil, soil_texture_set};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_TERRAIN_WITNESS_PATHS, then pass --ignored"]
fn installed_terrain_definition_families_have_the_observed_shapes() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let build = archive.build().ok_or("installed archive identity")?;
    assert_eq!(build.change_number(), "13189690", "exact PTU build");
    let paths = std::env::var_os("P4KX_TERRAIN_WITNESS_PATHS").ok_or("private selector list")?;
    let selectors: HashSet<_> = std::fs::read_to_string(paths)?
        .lines()
        .map(crate::starmap_index::key)
        .collect();
    assert_eq!(selectors.len(), 66, "exact scoped Archive selectors");
    let watch = ArchiveWatch::new(&archive);
    let mut census = Census::default();
    for entry in archive
        .entries()
        .iter()
        .filter(|entry| selectors.contains(&crate::starmap_index::key(entry.path())))
    {
        watch.poll()?;
        let mut raw = Vec::new();
        archive.open_entry_verified(entry)?.read_to_end(&mut raw)?;
        let json: Value = serde_json::from_slice(&raw)?;
        census.observe(entry.path(), &json)?;
    }
    watch.check()?;
    assert_eq!(
        census,
        Census {
            geology: 6,
            geology_textures: 3,
            soils: 9,
            soil_textures: 2,
            shading: 13,
            ecosystems: 33,
            scattered_slots: 91,
            ground_layers: 44,
            medium_height_scales: 16,
            empty_audio_arrays: 66
        },
        "independent original source shape",
    );
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Census {
    geology: usize,
    geology_textures: usize,
    soils: usize,
    soil_textures: usize,
    shading: usize,
    ecosystems: usize,
    scattered_slots: usize,
    ground_layers: usize,
    medium_height_scales: usize,
    empty_audio_arrays: usize,
}

impl Census {
    fn observe(&mut self, path: &str, json: &Value) -> TestResult {
        let guid = json["GUID"].as_str().ok_or("authored GUID")?;
        let guid = guid
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .unwrap_or(guid);
        let _guid: p4kx_extraction::Guid = guid.parse()?;
        if geology_set::FAMILY.includes(path) {
            self.geology += 1;
            self.scattered_slots += slots(json, "/scatteredAssets")?;
            assert_eq!(slots(json, "/cliffAssets")?, 0);
        } else if geology_texture_set::FAMILY.includes(path) {
            self.geology_textures += 1;
            self.ground_layers += slots(json, "/groundLayers")?;
        } else if soil::FAMILY.includes(path) {
            self.soils += 1;
            assert!(json["soilTextureSetGUID"].as_str().is_some());
            assert!(json["oceanSecondaryColourWeight"].is_number());
        } else if soil_texture_set::FAMILY.includes(path) {
            self.soil_textures += 1;
            self.ground_layers += slots(json, "/groundLayers")?;
            self.medium_height_scales += slots(json, "/mediumLayerHeightScales")?;
        } else if shading_class::FAMILY.includes(path) {
            self.shading += 1;
            assert!(json["groundLayer"].is_object());
            self.ground_layers += 1;
        } else if ecosystem::FAMILY.includes(path) {
            self.ecosystems += 1;
            for pointer in [
                "/SoundEffect/audioRTPCArray",
                "/SoundEffect/audioSwitchArray",
            ] {
                assert_eq!(slots(json, pointer)?, 0);
                self.empty_audio_arrays += 1;
            }
        } else {
            return Err("selector outside scoped terrain families".into());
        }
        Ok(())
    }
}

fn slots(json: &Value, pointer: &str) -> TestResult<usize> {
    json.pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| "expected source array".into())
}

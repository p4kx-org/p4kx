// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The relief-decal rule on the ship material files of a real Archive: decals with a grey mask hide,
//! decals with a coloured diffuse map (buttons, stitching, damage) are drawn (ADR-0225).

#![expect(clippy::print_stdout, reason = "the judged materials are the output")]

use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::path::Path;

use p4kx_extraction::model::{GREY_PROBE_MIP, LayerCache, MAX_GREY_SATURATION, Material};
use p4kx_extraction::{Archive, ModelSource, PathIndex, TextureVerdicts, probe_saturation};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const SHIP_FILES: &str = "data\\objects\\spaceships\\";
const GREY_MASK: &str = "rsi_pom_decals_diff";
const COLOURED_MAPS: [&str; 4] = [
    "anvl_switches_buttons_diff",
    "aegs_switches_buttons_diff",
    "gen_pom_metal_dmg_atlas_01_diff",
    "behr_trim_1_a_diff",
];
const MIN_JUDGED: usize = 20;
const GREY_MEASURED_UP_TO: f32 = 0.032;
const COLOURED_MEASURED_FROM: f32 = 0.076;

/// A relief-decal candidate as judged: its material, its diffuse map and whether the rule hides it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Judged {
    material: String,
    map: String,
    hidden: bool,
}

pub(super) fn ship_libraries(archive: &Archive) -> Vec<(String, String)> {
    archive
        .entries()
        .iter()
        .filter(|entry| {
            let lower = entry.path().to_ascii_lowercase();
            lower.starts_with(SHIP_FILES)
                && Path::new(&lower)
                    .extension()
                    .is_some_and(|extension| extension == "mtl")
        })
        .filter_map(|entry| {
            let library = entry
                .path()
                .strip_prefix("Data\\")?
                .strip_suffix(".mtl")?
                .replace('\\', "/");
            Some((entry.path().to_owned(), library))
        })
        .collect()
}

fn diffuse_file(material: &Material) -> Option<&str> {
    material
        .textures
        .iter()
        .find(|texture| texture.map.eq_ignore_ascii_case("TexSlot1"))
        .map(|texture| texture.file.as_str())
}

fn map_stem(file: &str) -> String {
    file.rsplit(['/', '\\'])
        .next()
        .and_then(|name| name.split('.').next())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn coloured_relief_candidates_are_drawn_and_grey_ones_are_hidden() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);
    let verdicts = TextureVerdicts::new();
    let mut judged: BTreeMap<Judged, usize> = BTreeMap::new();
    for (entry_path, library) in ship_libraries(&archive) {
        let status = ModelSource::material_library(
            &index,
            &entry_path,
            &library,
            &LayerCache::new(),
            &verdicts,
        );
        for material in status
            .library()
            .iter()
            .flat_map(|library| &library.materials)
        {
            let Some(file) =
                diffuse_file(material).filter(|_| material.diffuse_map_is_grey.is_some())
            else {
                continue;
            };
            let key = Judged {
                material: material.name.clone(),
                map: map_stem(file),
                hidden: material.is_relief_decal(),
            };
            *judged.entry(key).or_default() += 1;
        }
    }
    for (key, files) in &judged {
        println!(
            "{files:4} x {} {} hidden {}",
            key.material, key.map, key.hidden
        );
    }
    assert!(judged.len() >= MIN_JUDGED, "only {} judged", judged.len());
    assert!(
        judged.keys().any(|key| key.map == GREY_MASK && key.hidden),
        "{GREY_MASK} is hidden"
    );
    for coloured in COLOURED_MAPS {
        let hidden = judged.keys().any(|key| key.map == coloured && key.hidden);
        assert!(!hidden, "{coloured} is drawn");
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_grey_limit_lies_in_the_gap_between_grey_and_coloured_maps() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);
    let mut grey_most = 0.0_f32;
    let mut coloured_least = f32::MAX;
    let mut measured: HashSet<String> = HashSet::new();
    let verdicts = TextureVerdicts::new();
    for (entry_path, library) in ship_libraries(&archive) {
        let status = ModelSource::material_library(
            &index,
            &entry_path,
            &library,
            &LayerCache::new(),
            &verdicts,
        );
        for material in status
            .library()
            .iter()
            .flat_map(|library| &library.materials)
        {
            let (Some(file), Some(grey)) = (diffuse_file(material), material.diffuse_map_is_grey)
            else {
                continue;
            };
            if !measured.insert(file.to_ascii_lowercase()) {
                continue;
            }
            let Some(saturation) = probe_saturation(&index, file) else {
                continue;
            };
            if grey {
                grey_most = grey_most.max(saturation);
            } else {
                coloured_least = coloured_least.min(saturation);
            }
        }
    }
    println!("probe mip {GREY_PROBE_MIP}: grey up to {grey_most}, coloured from {coloured_least}");
    assert!(
        grey_most <= GREY_MEASURED_UP_TO,
        "grey maps measure up to {grey_most}"
    );
    assert!(
        coloured_least >= COLOURED_MEASURED_FROM,
        "coloured maps measure from {coloured_least}"
    );
    assert!(
        (GREY_MEASURED_UP_TO..COLOURED_MEASURED_FROM).contains(&MAX_GREY_SATURATION),
        "the limit {MAX_GREY_SATURATION} lies in the gap"
    );
    Ok(())
}

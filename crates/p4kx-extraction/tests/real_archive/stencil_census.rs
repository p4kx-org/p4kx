// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The stencil decals of a real Archive: where their stencil map sits, what it is called and which
//! parameters they set; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::BTreeMap;
use std::error::Error;
use std::io::Read;

use p4kx_extraction::model::{Material, MaterialLibrary, TextureRole, texture_suffix_lowercase};
use p4kx_extraction::{Archive, Entry};
use p4kx_testkit::{real_archive_key, real_archive_path};
use rayon::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// Returns what draws `material`, a stencil decal: exactly one of its stencil map, its diffuse map
/// or nothing; `None` when the rules overlap or leave a gap.
fn classes(material: &Material) -> Option<&'static str> {
    match (
        material.stencil_texture().is_some(),
        material.has_diffuse_map(),
        material.is_stencil_without_motif(),
    ) {
        (true, false, false) => Some("by_stencil_map"),
        (false, true, false) => Some("by_diffuse_map"),
        (false, false, true) => Some("no_motif"),
        _ => None,
    }
}

/// Returns the shape of `material`: what draws it, its tint parameter, palette, slots with their
/// roles.
fn shape(material: &Material) -> String {
    let slots: Vec<String> = material
        .textures
        .iter()
        .map(|texture| {
            let kind = if texture.file.starts_with('$') {
                texture.file.clone()
            } else if material.texture_role(&texture.map, &texture.file) == TextureRole::Stencil {
                format!("Stencil(_{})", texture_suffix_lowercase(&texture.file))
            } else {
                format!("{:?}", material.texture_role(&texture.map, &texture.file))
            };
            format!("{}={kind}", texture.map)
        })
        .collect();
    let stencil = material.stencil();
    format!(
        "{} {} tint_override={:?} palette={} layers={} colour={} [{}]",
        material.shader,
        classes(material).unwrap_or("INVALID"),
        stencil.map(|stencil| stencil.palette_tint),
        material.palette.is_some(),
        material.layers.len(),
        stencil.is_some_and(|stencil| stencil.colour.is_some()),
        slots.join(" ")
    )
}

#[test]
#[expect(
    clippy::print_stdout,
    reason = "the census table is the measurement; the test prints it"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_stencil_decal_is_drawn_by_exactly_one_of_its_maps_or_nothing() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let mtl: Vec<&Entry> = archive
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".mtl"))
        .collect();

    let census = mtl
        .par_iter()
        .fold(BTreeMap::<String, usize>::new, |mut census, entry| {
            let mut bytes = Vec::new();
            let read = archive
                .open_entry(entry)
                .ok()
                .and_then(|mut reader| reader.read_to_end(&mut bytes).ok());
            if let (Some(_), Ok(library)) = (read, MaterialLibrary::parse(entry.path(), &bytes)) {
                for material in library
                    .materials
                    .iter()
                    .filter(|material| material.is_stencil_decal())
                {
                    *census.entry(shape(material)).or_default() += 1;
                }
            }
            census
        })
        .reduce(BTreeMap::new, |mut all, part| {
            for (key, count) in part {
                *all.entry(key).or_default() += count;
            }
            all
        });

    for (key, count) in &census {
        println!("{count:>6}  {key}");
    }
    assert!(!census.is_empty(), "the Archive has stencil decals");
    assert!(
        census.keys().all(|key| !key.contains(" INVALID ")),
        "every stencil decal is drawn by exactly one of its stencil map, its diffuse map or nothing"
    );
    Ok(())
}

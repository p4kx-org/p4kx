// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Census of the creature Entities that name an animation database: how many clips are found
//! next to the skeleton, how many fit it and whether the GLB with them is valid.

#![expect(clippy::print_stdout, reason = "the census is the output")]

use std::collections::BTreeMap;
use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, EntityNode, ForgePalettes,
    LoadoutResolver, creature_animations,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

/// What the creatures of one animation database came to: the most of each count over them.
#[derive(Default)]
struct Row {
    entities: usize,
    read: usize,
    fitted: usize,
    written: usize,
    unavailable: Vec<String>,
    largest: u64,
}

/// What one creature came to.
struct Observed {
    read: usize,
    fitted: usize,
    written: usize,
    bytes: u64,
    unavailable: Vec<String>,
}

impl Row {
    fn add(&mut self, observed: Observed) {
        self.entities += 1;
        self.read = self.read.max(observed.read);
        self.fitted = self.fitted.max(observed.fitted);
        self.written = self.written.max(observed.written);
        self.largest = self.largest.max(observed.bytes);
        for text in observed.unavailable {
            if !self.unavailable.contains(&text) {
                self.unavailable.push(text);
            }
        }
    }
}

/// Reads the clips of the creature `tree`, writes its GLB with them and checks the GLB.
fn observe(
    tree: &EntityNode,
    models: &ArchiveModels<'_>,
    palettes: ForgePalettes<'_>,
) -> Result<Observed, Box<dyn Error>> {
    let assembly =
        Assembly::build_with_palettes(tree, models, &palettes, &AssemblyOptions::default())?;
    let clips = creature_animations(models, &assembly, tree)?;
    let fitted = assembly.animate(&clips.set.animations);
    let mut bytes = Vec::new();
    let summary = assembly.write_glb_animated(&fitted, &GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes).map_err(|error| format!("{}: {error}", tree.class_name()))?;
    let written = glb.document.animations().len();
    let counted = summary
        .animations
        .iter()
        .filter(|animation| animation.channels > 0)
        .count();
    assert_eq!(
        written,
        counted,
        "{}: the summary counts the written animations",
        tree.class_name()
    );
    Ok(Observed {
        read: clips.set.animations.len(),
        fitted: fitted.len(),
        written,
        bytes: summary.bytes,
        unavailable: clips
            .issues
            .iter()
            .map(|issue| issue.kind().to_string())
            .collect(),
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_creature_that_names_an_animation_database_writes_a_valid_glb_with_its_clips() -> TestResult
{
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let mut without_database = 0;
    for record in forge.records().filter(|record| {
        record
            .type_name()
            .is_ok_and(|name| name == p4kx_assembly::ENTITY_TYPE)
    }) {
        let Ok(tree) = resolver.resolve(&record) else {
            continue;
        };
        if !tree
            .geometry()
            .is_some_and(p4kx_assembly::Geometry::is_rigged_file_outside_human_rigs)
        {
            continue;
        }
        let Some(database) = tree.animation_database() else {
            without_database += 1;
            continue;
        };
        rows.entry(database.to_ascii_lowercase())
            .or_default()
            .add(observe(&tree, &models, palettes)?);
    }

    println!("creatures without a database: {without_database}");
    println!("database: entities, clips read, fitted, written, largest GLB bytes");
    for (database, row) in &rows {
        println!(
            "{database}: {}, {}, {}, {}, {}",
            row.entities, row.read, row.fitted, row.written, row.largest
        );
        for text in &row.unavailable {
            println!("    {text}");
        }
    }
    assert!(
        rows.values().any(|row| row.written > 0),
        "a creature whose animations are written"
    );
    Ok(())
}

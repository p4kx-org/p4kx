// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_settings::ModelMaterials;
use p4kx_testkit::{ArchiveBuilder, hologram_suppression_entries};

use super::{MaterialSuppression, MaterialSuppressionEffect, MaterialSuppressionReason};
use crate::assembly::{AssemblyPieceSource, AssemblyScene, BuildOptions, Hooks};

fn assembly() -> Result<Assembly, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    hologram_suppression_entries()
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find("TEST_Ship")?;
    let tree = resolver.resolve(&entity)?;
    Ok(Assembly::build(
        &tree,
        &ArchiveModels::new(&archive),
        &AssemblyOptions::default(),
    )?)
}

#[test]
fn observing_material_suppression_preserves_the_normal_render_buffers() -> Result<(), Box<dyn Error>>
{
    let assembly = assembly()?;
    let options = BuildOptions {
        materials: ModelMaterials::Colours,
        textures: None,
    };
    let hooks = Hooks {
        cancelled: &|| false,
        progress: &|_| {},
    };
    let ordinary = AssemblyScene::build(&assembly, &options, &hooks)?;
    let observed = AssemblyScene::build_observed(&assembly, &options, &hooks)?;

    assert_eq!(
        ordinary.vertices(),
        observed.vertices(),
        "flattened vertices"
    );
    assert_eq!(
        ordinary.indices(),
        observed.indices(),
        "flattened triangles"
    );
    assert_eq!(
        ordinary.groups(),
        observed.groups(),
        "render groups and order"
    );
    assert_eq!(
        ordinary.items(),
        observed.items(),
        "draw items and materials"
    );
    assert_eq!(
        ordinary.draws(),
        observed.draws(),
        "placements and draw ranges"
    );
    assert_eq!(ordinary.bounds(), observed.bounds(), "camera framing");
    assert_eq!(
        ordinary.material_suppressions().count(),
        0,
        "normal builds need no receipts"
    );
    assert_eq!(observed.drawn_parts(), 2, "only the gun and relay remain");

    assert_slots(&observed);
    Ok(())
}

fn assert_slots(observed: &AssemblyScene) {
    let uses: Vec<_> = observed.material_suppressions().collect();
    assert!(
        uses.iter().all(|used| used.slot() != 0),
        "an unused slot cannot suppress a range"
    );
    assert_eq!(
        uses.iter()
            .filter(|used| used.effect() == MaterialSuppressionEffect::WholePiece)
            .map(|used| (used.source(), used.slot(), used.reason()))
            .collect::<Vec<_>>(),
        [
            (
                AssemblyPieceSource::Part(0),
                1,
                MaterialSuppressionReason::Hidden
            ),
            (
                AssemblyPieceSource::Part(0),
                2,
                MaterialSuppressionReason::Decal
            ),
            (
                AssemblyPieceSource::Part(1),
                1,
                MaterialSuppressionReason::Hidden
            ),
            (
                AssemblyPieceSource::Part(1),
                2,
                MaterialSuppressionReason::Decal
            ),
        ],
        "actual whole-piece slots and rules",
    );
    assert_eq!(
        uses.into_iter()
            .filter(|used| used.effect() == MaterialSuppressionEffect::DrawRange)
            .map(MaterialSuppression::slot)
            .collect::<Vec<_>>(),
        [1, 1],
        "the admitted pieces still omit the decal range",
    );
}

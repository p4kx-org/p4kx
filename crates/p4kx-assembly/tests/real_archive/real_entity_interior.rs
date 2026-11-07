// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Interior of real ships: the rooms land inside the hull's bounds.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, EntityInterior, InteriorOptions,
    InteriorPartKind, LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_model::{Model, Transform};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::real_container_census::{bone_translation, near};

type TestResult = Result<(), Box<dyn Error>>;

const MARGIN: f32 = 2.0;

pub(super) type Bounds = ([f32; 3], [f32; 3]);

fn apply(chain: &Transform, [x, y, z]: [f32; 3]) -> [f32; 3] {
    chain
        .rows
        .map(|[a, b, c, d]| a.mul_add(x, b.mul_add(y, c.mul_add(z, d))))
}

pub(super) fn hull_bounds(model: &Model) -> Option<Bounds> {
    let mut bounds: Option<Bounds> = None;
    for mesh in &model.lods.first()?.meshes {
        let mut chain = Transform::IDENTITY;
        let mut at = Some(mesh.node);
        while let Some(index) = at {
            let node = model.nodes.get(index)?;
            chain = node.transform.then(&chain);
            at = node.parent;
        }
        for position in &mesh.positions {
            let point = apply(&chain, *position);
            let (min, max) = bounds.get_or_insert((point, point));
            for ((low, high), value) in min.iter_mut().zip(max.iter_mut()).zip(point) {
                *low = low.min(value);
                *high = high.max(value);
            }
        }
    }
    bounds
}

fn inside(point: [f32; 3], (min, max): &Bounds) -> bool {
    point
        .iter()
        .zip(min.iter().zip(max))
        .all(|(v, (low, high))| *v >= low - MARGIN && *v <= high + MARGIN)
}

fn rooms_outside_the_hull(class_name: &str) -> Result<(usize, usize), Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(class_name)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let interior = EntityInterior::build(&archive, &assembly, &tree, &InteriorOptions::default())?;
    placement_against_hull(&assembly, &interior)
}

pub(super) fn placement_against_hull(
    assembly: &Assembly,
    interior: &EntityInterior,
) -> Result<(usize, usize), Box<dyn Error>> {
    let hull = assembly
        .root()
        .and_then(|part| part.model())
        .and_then(hull_bounds)
        .ok_or("the hull has no geometry")?;
    let hull_model = assembly.root().and_then(|part| part.model());
    let (mut placed, mut outside) = (0, 0);
    for container in interior.containers() {
        let reference = container.reference();
        let bone = hull_model.and_then(|model| bone_translation(model, reference.bone()));
        let expected = match bone {
            Some(at) if !near(at, [0.0; 3]) => at,
            _ => reference.position(),
        };
        crate::media_fixtures::require(
            near(container.placement().translation(), expected),
            crate::media_fixtures::Check::ContainerPlacement,
        )?;
        let models = container
            .interior()
            .parts()
            .iter()
            .filter(|part| part.kind() == InteriorPartKind::Model);
        for part in models {
            let point = container.placement().then(part.world()).translation();
            placed += 1;
            outside += usize::from(!inside(point, &hull));
        }
    }
    Ok((placed, outside))
}

fn check(class_name: &str, least: usize) -> TestResult {
    let (placed, outside) = rooms_outside_the_hull(class_name)?;
    assert!(placed >= least, "{class_name}: {placed} models placed");
    assert_eq!(outside, 0, "{outside} of {placed} models of {class_name}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_constellation_has_its_rooms_inside_the_hull() -> TestResult {
    check("RSI_Constellation_Andromeda", 50)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_cutlass_has_its_rooms_inside_the_hull() -> TestResult {
    check("DRAK_Cutlass_Black", 5)
}

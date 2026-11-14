// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, Attachment, LoadoutResolver, Placement,
};
use p4kx_extraction::model::{Model, Transform};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const RELAY: &str = "RELAY_3slot";
const LOD: u32 = 2;
const MATRIX_EPSILON: f32 = 1e-5;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn flagged_relay_fuses_keep_the_static_socket_transform() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let data = DataForgeEntry::read(&archive)?;
    let forge = data.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(RELAY)?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default().with_lod(LOD))?;
    let relay = assembly.root().ok_or("static relay root")?;
    let model = relay.model().ok_or("static relay model")?;
    let mut rotated = 0_usize;
    for port in tree.ports() {
        let Attachment::Entity(entity) = port.attachment() else {
            continue;
        };
        if !port
            .definition()
            .is_some_and(p4kx_assembly::PortDefinition::no_rotation)
        {
            continue;
        }
        let part = assembly
            .part_at(port.name())
            .ok_or("attached static fuse")?;
        assert_eq!(
            part.guid(),
            entity.guid(),
            "the original loadout supplies the fuse"
        );
        assert!(
            part.model().is_some(),
            "the fuse geometry is used for placement verification"
        );
        let node_name = match part.placement() {
            Placement::PortNode => port.name(),
            Placement::Helper(name) => name,
            _ => return Err("a flagged fuse has no physical socket node".into()),
        };
        let socket = socket_transform(model, node_name)?;
        assert!(
            same_transform(part.local(), &socket),
            "the fuse fits the physical socket"
        );
        let translation_only = p4kx_assembly::offset_transform(socket.translation(), [0.0; 3]);
        if !same_transform(&socket, &translation_only) {
            rotated += 1;
        }
    }
    assert!(
        rotated > 0,
        "at least one flagged fuse distinguishes full rotation from translation only"
    );
    watch.check()?;
    Ok(())
}

fn socket_transform(model: &Model, name: &str) -> TestResult<Transform> {
    let mut candidates = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, row)| row.name.eq_ignore_ascii_case(name));
    let (selected, _) = candidates.next().ok_or("a source socket node")?;
    assert!(
        candidates.next().is_none(),
        "one socket owns the compared transform"
    );
    let mut lineage: Vec<_> = model
        .node_ancestors(selected)
        .map(|(_, node)| node.transform)
        .collect();
    lineage.reverse();
    Ok(lineage
        .into_iter()
        .fold(Transform::IDENTITY, |world, node| world.then(&node)))
}

fn same_transform(left: &Transform, right: &Transform) -> bool {
    left.rows
        .iter()
        .flatten()
        .zip(right.rows.iter().flatten())
        .all(|(a, b)| (a - b).abs() < MATRIX_EPSILON)
}

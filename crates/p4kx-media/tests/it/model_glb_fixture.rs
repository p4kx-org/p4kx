// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, TestEntry, TestModel, TestMtl, TestMtlMaterial, TestNode, TestPositions,
    TestSkeleton, test_key,
};

pub(super) const PRIMARY: &str = "Data/Synthetic/example.skin";
const GEOMETRY: &str = "Data/Synthetic/example.skinm";
const LOD_MODEL: &str = "Data/Synthetic/example_lod1.skin";
const LOD_GEOMETRY: &str = "Data/Synthetic/example_lod1.skinm";
pub(super) const HELPER: &str = "Mount_Helper";
const UNKNOWN: u32 = 0x0BAD_BEEF;

pub(super) struct Fixture {
    pub(super) base: TestModel,
    pub(super) level_one: TestModel,
    material: Vec<u8>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let nodes = [
            TestNode {
                name: HELPER.to_owned(),
                parent: None,
                local: [
                    [0.0, -1.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0, 2.0],
                    [0.0, 0.0, 1.0, 3.0],
                ],
                kind: 2,
            },
            TestNode {
                name: "Mesh_Node".to_owned(),
                parent: Some(0),
                local: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                ],
                kind: 0,
            },
        ];
        let mesh = TestModel::cube().with_positions(TestPositions::Float);
        let level_one = mesh
            .clone()
            .scaled(0.5)
            .with_wide_indices()
            .with_index(0, 1)
            .with_index(1, 2)
            .with_index(2, 0)
            .with_skeleton(&TestSkeleton::chain(6))
            .with_bone_map(false, vec![vec![(5, 255)]; 24]);
        let base = mesh
            .with_index(0, 2)
            .with_index(1, 0)
            .with_index(2, 1)
            .with_nodes(Vec::from(nodes))
            .with_node_details(0, [1.0; 3], &[("component", "inspection")])
            .with_skeleton(&TestSkeleton::two_bones())
            .with_bone_map(false, vec![vec![(1, 255)]; 24])
            .with_chunk(ivo::PHYSICAL_HIERARCHY.id(), b"opaque fixture")
            .with_chunk(UNKNOWN, b"unknown fixture");
        let material = TestMtl::multi(&[
            TestMtlMaterial::new("unused_fixture"),
            TestMtlMaterial::new("caps_fixture"),
            TestMtlMaterial::new("sides_fixture"),
        ])
        .cryxml();
        Self {
            base,
            level_one,
            material,
        }
    }

    pub(super) fn archive(&self, folder: &Path) -> Result<Archive, Box<dyn Error>> {
        let multi = self.base.clone().with_lod(self.level_one.clone());
        let entries = [
            TestEntry::stored(PRIMARY, multi.cgf()),
            TestEntry::stored(GEOMETRY, multi.cgfm()),
            TestEntry::stored(LOD_MODEL, multi.lod_cgf(1).ok_or("LOD1 metadata")?),
            TestEntry::zstd(LOD_GEOMETRY, multi.lod_cgfm(1).ok_or("LOD1 geometry")?).encrypted(),
            TestEntry::stored("Data/objects/test/cube.mtl", self.material.as_slice()),
        ];
        let path = folder.join("Synthetic.p4k");
        entries
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&path)?;
        Ok(Archive::open_with(
            path,
            OpenOptions::new().key(Some(test_key())),
        )?)
    }
}

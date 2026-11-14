// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel, TestMtl,
    TestMtlMaterial, TestPort, TestSubmesh, entity_schema,
};

const ROOT_MODEL: &str = "Objects/Test/root.cgf";
const SIBLING_MODEL: &str = "Objects/Test/sibling.cgf";
const CHILD_OFFSET: [f32; 3] = [6.0, 0.0, 0.0];

/// Returns two attached models whose rooted and sibling libraries share one bare name.
#[must_use]
pub fn material_identity_entries(rooted: &str) -> Vec<TestEntry> {
    let root = TestEntity::new(1, "TEST_RootMaterial", "entities/root.xml")
        .geometry(ROOT_MODEL, None)
        .port(TestPort::new("child").helper("cube", CHILD_OFFSET, [0.0; 3]))
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "child",
            "TEST_SiblingMaterial",
        )]));
    let child = TestEntity::new(2, "TEST_SiblingMaterial", "entities/sibling.xml")
        .geometry(SIBLING_MODEL, None);
    let database = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .build();
    let root_model = model(rooted);
    let sibling_model = model("paint");
    vec![
        TestEntry::stored("Data/Game2.dcb", database),
        TestEntry::stored(format!("Data/{ROOT_MODEL}"), root_model.cgf()),
        TestEntry::stored(format!("Data/{ROOT_MODEL}m"), root_model.cgfm()),
        TestEntry::stored(format!("Data/{SIBLING_MODEL}"), sibling_model.cgf()),
        TestEntry::stored(format!("Data/{SIBLING_MODEL}m"), sibling_model.cgfm()),
        TestEntry::stored(
            "Data/paint.mtl",
            TestMtl::multi(&[TestMtlMaterial::new("root_paint").diffuse([0.8, 0.1, 0.1])]).cryxml(),
        ),
        TestEntry::stored(
            "Data/Objects/Test/paint.mtl",
            TestMtl::multi(&[TestMtlMaterial::new("sibling_paint").diffuse([0.2, 0.4, 0.6])])
                .cryxml(),
        ),
    ]
}

fn model(reference: &str) -> TestModel {
    TestModel::cube()
        .with_material_library(reference)
        .with_submeshes(vec![TestSubmesh {
            material_id: 0,
            node: 1,
            first_index: 0,
            index_count: 36,
            first_vertex: 0,
            vertex_count: 24,
        }])
}

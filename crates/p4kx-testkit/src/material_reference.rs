// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestEntry, TestModel, TestMtl, TestMtlMaterial};

/// The model Entry in the material-reference fixture.
pub const MATERIAL_REFERENCE_MODEL: &str = "Data/Objects/Test/cube.cgf";

/// Returns a cube and an optional two-colour library for material-reference tests.
#[must_use]
pub fn material_reference_entries(reference: &str, target: Option<&str>) -> Vec<TestEntry> {
    let model = TestModel::cube().with_material_library(reference);
    let mut entries = vec![
        TestEntry::stored(MATERIAL_REFERENCE_MODEL, model.cgf()),
        TestEntry::stored("Data/Objects/Test/cube.cgfm", model.cgfm()),
    ];
    if let Some(target) = target {
        let materials = [
            TestMtlMaterial::new("unused"),
            TestMtlMaterial::new("caps").diffuse([0.8, 0.1, 0.1]),
            TestMtlMaterial::new("sides")
                .diffuse([0.2, 0.4, 0.6])
                .opacity(0.5),
        ];
        entries.push(TestEntry::stored(
            target,
            TestMtl::multi(&materials).cryxml(),
        ));
    }
    entries
}

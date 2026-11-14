// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_character::{Chf, NameTable, diff_chf, diff_chf_observed};
use p4kx_testkit::{TestChf, TestMaterial};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn observed_diff_uses_actual_pair_positions_for_duplicate_hashes() -> TestResult {
    let sample = |flags: [u32; 2]| {
        let mut chf = TestChf::v7();
        chf.materials = flags
            .into_iter()
            .map(|flags| TestMaterial {
                attachment: 17,
                base_material: [2; 16],
                flags,
                sub_materials: Vec::new(),
            })
            .collect();
        chf
    };
    let old = Chf::read(&sample([0, 1]).to_bytes())?;
    let new = Chf::read(&sample([2, 0]).to_bytes())?;
    let names = NameTable::builtin();
    let observed = diff_chf_observed(&old, &new, &names);
    let legacy_rows: Vec<_> = observed
        .iter()
        .map(|change| change.change().clone())
        .collect();
    assert_eq!(
        legacy_rows,
        diff_chf(&old, &new, &names),
        "preserve legacy report order and text"
    );
    let flags_row = observed
        .iter()
        .find(|change| change.change().raw_path == "materials.0x00000011#1.flags")
        .ok_or("missing changed flags")?;
    assert_eq!(flags_row.before().pointer(), "/character/materials/1/flags");
    assert_eq!(flags_row.after().pointer(), "/character/materials/0/flags");
    let order = observed
        .iter()
        .find(|change| change.change().raw_path == "materials.order")
        .ok_or("missing order change")?;
    assert_eq!(order.before().pointer(), "/character/materials");
    assert_eq!(order.after().pointer(), "/character/materials");
    Ok(())
}

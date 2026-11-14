// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;

use p4kx_character::{Chf, ChfJsonNameUse, ChfNameKind, NameTable, lowered_hash};
use p4kx_testkit::{TestChf, TestItemPort};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn observed_named_json_keeps_existing_bytes_and_exact_emitted_fields() -> TestResult {
    let mut input = TestChf::v7();
    input.materials.clear();
    let hash = lowered_hash("head_itemport");
    input.ports = Some(TestItemPort {
        port: hash,
        item: [1; 16],
        tail_count: 0,
        children: Vec::new(),
    });
    let chf = Chf::read(&input.to_bytes())?;
    let names = NameTable::builtin();
    let mut fields = BTreeMap::new();
    let actual = chf.to_json_named_observed(&names, |field| {
        assert!(
            fields
                .insert(
                    field.pointer().to_owned(),
                    field
                        .name()
                        .filter(|name| name.usage() == ChfJsonNameUse::Label)
                )
                .is_none(),
            "one emitted datum per pointer"
        );
    })?;
    assert_eq!(actual, chf.to_json_named(&names)?);
    let tree: Value = serde_json::from_str(&actual.0)?;
    for (pointer, label) in &fields {
        let value = tree
            .pointer(pointer)
            .ok_or("an observed datum was not emitted")?;
        if let Some(label) = label {
            assert_eq!(label.kind(), ChfNameKind::Port);
            assert_eq!(label.hash(), hash);
            assert_eq!(value, "head_itemport");
        }
    }
    assert_eq!(fields.get("/character/item_ports/port"), Some(&None));
    assert!(
        fields
            .get("/character/item_ports/port_label")
            .is_some_and(Option::is_some)
    );
    assert_eq!(fields.get("/character/materials"), Some(&None));
    assert_eq!(fields.get("/container/frame"), Some(&None));
    Ok(())
}

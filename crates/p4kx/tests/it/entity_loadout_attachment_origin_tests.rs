// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    entity_schema,
};
use serde_json::Value;

use super::{TestResult, field_uses, origin, original_bytes, verify_emitted_fields};

#[test]
fn json_keeps_stored_reference_and_actual_operator_match() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let ship = TestEntity::new(1, "Root", "entities/root.xml").loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::reference("linked", 2).named("Other"),
        TestLoadoutEntry::reference("missing", 0xA100),
        TestLoadoutEntry::empty("mount").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("nested", "Part"),
        ])),
        TestLoadoutEntry::reference("replace", 2),
    ]));
    let dataforge = entity_schema(DataForgeBuilder::new())
        .record(ship.record())
        .record(TestEntity::new(2, "Part", "entities/part.xml").record())
        .record(TestEntity::new(3, "Other", "entities/other.xml").record())
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", dataforge.clone()).encrypted())
        .write_to(&archive)?;
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "entity", "loadout"])
        .arg(&archive)
        .args([
            "Root",
            "--exclude",
            "MOUNT/Nested",
            "--replace",
            "REPLACE=Other",
        ])
        .output()?;
    assert!(
        result.status.success(),
        "synthetic loadout resolves: {result:?}"
    );
    let json: Value = serde_json::from_slice(&result.stdout)?;
    verify_reference(&json)?;
    verify_operators(&json, &dataforge)?;
    verify_emitted_fields(&json)
}

fn verify_reference(json: &Value) -> TestResult {
    assert_eq!(
        json.pointer("/entity/ports/0/entity/class_name")
            .and_then(Value::as_str),
        Some("Part"),
        "the stored Reference wins over the adjacent class spelling"
    );
    let attached = "/entity/ports/0/entity/guid";
    let missing = "/entity/ports/1/error/reference";
    let mut ordinals = Vec::new();
    for (pointer, role) in [(attached, "selector"), (missing, "payload")] {
        let receipt = origin(json, pointer)?;
        assert_eq!(receipt["sourceSetComplete"], true, "{pointer}");
        assert_eq!(receipt["containsEncryptedSource"], true, "{pointer}");
        let site = field_uses(json, pointer)?
            .into_iter()
            .find(|site| {
                site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                    && site["role"] == role
            })
            .ok_or("the actual Reference getter")?;
        assert_eq!(
            site.pointer("/location/indices/0").and_then(Value::as_str),
            Some("0"),
            "the selected Reference belongs to the root Record"
        );
        ordinals.push(
            site.pointer("/location/indices/1")
                .and_then(Value::as_str)
                .ok_or("source ordinal")?,
        );
    }
    let [attached, missing] = ordinals.as_slice() else {
        return Err("one source occurrence for each Reference".into());
    };
    assert_ne!(
        attached, missing,
        "each Reference keeps its own source occurrence"
    );
    let reason = origin(json, "/entity/ports/1/error/reason")?;
    assert_eq!(
        reason["sourceSetComplete"], false,
        "the Reference is known while the failing lookup inventory is incomplete"
    );
    Ok(())
}

fn verify_operators(json: &Value, source: &[u8]) -> TestResult {
    let excluded = "/entity/ports/2/nested/0/source";
    let replaced = "/entity/ports/3/source";
    for (pointer, action, expected) in [
        (
            excluded,
            "excluded",
            &[b"mount".as_slice(), b"nested".as_slice()][..],
        ),
        (replaced, "replaced", &[b"replace".as_slice()][..]),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_str),
            Some(action),
            "{pointer} carries the selected action"
        );
        let receipt = origin(json, pointer)?;
        assert_eq!(receipt["sourceSetComplete"], true, "{pointer}");
        assert_eq!(receipt["containsEncryptedSource"], true, "{pointer}");
        let mut texts = field_uses(json, pointer)?
            .into_iter()
            .filter(|site| {
                site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
                    && site["role"] == "selector"
            })
            .map(|site| original_bytes(source, site))
            .collect::<Result<Vec<_>, _>>()?;
        texts.sort_unstable();
        assert_eq!(texts, expected, "actual normalized-match path for {action}");
    }
    assert_eq!(
        json.pointer("/entity/ports/3/entity/class_name")
            .and_then(Value::as_str),
        Some("Other"),
        "the operator's replacement supplies the attached Record"
    );
    Ok(())
}

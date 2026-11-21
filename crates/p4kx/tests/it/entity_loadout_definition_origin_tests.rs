// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, Value as DataForgeValue};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestPort, entity_schema,
};
use serde_json::Value;

use super::{TestResult, field_uses, origin, original_bytes, verify_emitted_fields};

#[path = "entity_loadout_rotation_origin_tests.rs"]
mod rotation;

const PORT: &str = "/entity/ports/0";
const VEHICLE_PATH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Probe.xml";
const LOADOUT: &str = concat!(
    "<Loadout><Items><Item portName='HARDPOINT' itemName=''/>",
    "<Item portName='unconfigured' itemName=''/></Items></Loadout>",
);

#[test]
fn json_definition_and_visibility_keep_the_actual_dataforge_port() -> TestResult {
    let (json, dataforge) = loadout(None)?;
    let name = format!("{PORT}/name");
    assert_eq!(
        origin(&json, &name)?["containsEncryptedSource"],
        false,
        "the clear XML selector still owns the emitted port name"
    );
    let missing = "/entity/ports/1/definition";
    assert_eq!(
        json.pointer(missing),
        Some(&Value::Null),
        "the absence stays explicit"
    );
    assert_eq!(
        origin(&json, missing)?["sourceSetComplete"],
        false,
        "the unrecorded negative definition selection remains incomplete"
    );
    verify_definition(&json, &dataforge)?;
    verify_emitted_fields(&json)
}

#[test]
fn xml_visibility_carries_the_vehicle_selector_and_excludes_unused_dataforge_flags() -> TestResult {
    let (json, dataforge) = loadout(Some(VEHICLE_PATH))?;
    let visibility = format!("{PORT}/visibility");
    assert_eq!(
        json.pointer(&visibility).and_then(Value::as_str),
        Some("visible"),
        "vehicle XML takes precedence"
    );
    assert_eq!(
        origin(&json, &visibility)?["containsEncryptedSource"],
        true,
        "the encrypted DataForge vehicle selector selected the clear XML"
    );
    assert_eq!(
        origin(&json, &visibility)?["sourceSetComplete"],
        false,
        "the first-matching effective Part survey remains incomplete"
    );
    let text: Vec<_> = field_uses(&json, &visibility)?
        .into_iter()
        .filter(|source| {
            source
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some("dataforgeDecodedBytes")
        })
        .map(|source| original_bytes(&dataforge, source))
        .collect::<Result<_, _>>()?;
    assert!(text.contains(&VEHICLE_PATH.as_bytes()));
    assert!(!text.contains(&b"invisible locked".as_slice()));
    Ok(())
}

fn loadout(vehicle: Option<&str>) -> Result<(Value, Vec<u8>), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let mut root = TestEntity::new(1, "Root", "entities/root.xml")
        .loadout(TestLoadout::Xml("Loadouts/Root.xml".to_owned()))
        .port(
            TestPort::new("Hardpoint")
                .flags("invisible locked")
                .sizes(2, 4)
                .item_type("WeaponGun", ["", "charged", "charged"])
                .helper("mount_helper", [1.0, 2.0, 3.0], [4.0, 5.0, 6.0])
                .item_helper("item_helper"),
        )
        .port(
            TestPort::new("hardpoint")
                .flags("wrong declaration")
                .sizes(7, 9),
        );
    if let Some(path) = vehicle {
        root = root.vehicle(path, "");
    }
    let dataforge = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .build();
    let mut entries = ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", dataforge.clone()).encrypted())
        .entry(TestEntry::stored(
            "Data\\Loadouts\\Root.xml",
            LOADOUT.as_bytes(),
        ));
    if let Some(path) = vehicle {
        entries = entries.entry(TestEntry::stored(
            format!("Data/{path}"),
            br#"<Vehicle name="Probe"><Parts><Part name="hardpoint" class="ItemPort"><ItemPort flags=""/></Part></Parts></Vehicle>"#,
        ));
    }
    entries.write_to(&archive)?;
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "entity", "loadout"])
        .arg(&archive)
        .arg("Root")
        .output()?;
    assert!(
        result.status.success(),
        "synthetic definition loads: {result:?}"
    );
    Ok((serde_json::from_slice(&result.stdout)?, dataforge))
}

fn verify_definition(json: &Value, dataforge: &[u8]) -> TestResult {
    for (member, expected, role) in [
        ("definition/flags", "invisible locked", "payload"),
        ("definition/display_name", "@LOC_EMPTY", "payload"),
        ("definition/types/0/type", "WeaponGun", "payload"),
        ("definition/types/0/sub_types/0", "charged", "payload"),
        ("definition/transform/helper", "mount_helper", "payload"),
        (
            "definition/transform/offset_helper",
            "item_helper",
            "payload",
        ),
        (
            "definition/transform/attachment",
            "SItemPortDefAttachmentImplementationBone",
            "conversion",
        ),
        ("visibility", "invisible locked", "conversion"),
    ] {
        let pointer = format!("{PORT}/{member}");
        let receipt = origin(json, &pointer)?;
        assert_eq!(receipt["containsEncryptedSource"], true, "{pointer}");
        assert_eq!(receipt["sourceSetComplete"], true, "{pointer}");
        verify_text_origin(json, dataforge, &pointer, expected, role)?;
    }
    assert_eq!(
        json.pointer(&format!("{PORT}/visibility"))
            .and_then(Value::as_str),
        Some("invisible"),
        "DataForge Flags control visibility without vehicle XML"
    );
    verify_numeric_origins(json, dataforge)?;
    verify_duplicate_subtypes(json)
}

fn verify_text_origin(
    json: &Value,
    dataforge: &[u8],
    pointer: &str,
    expected: &str,
    role: &str,
) -> TestResult {
    let siblings = field_uses(json, pointer)?;
    let text = siblings
        .iter()
        .find(|site| {
            site["role"] == role
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
        })
        .ok_or("original DataForge text use")?;
    assert_eq!(
        original_bytes(dataforge, text)?,
        expected.as_bytes(),
        "{pointer}"
    );
    Ok(())
}

fn verify_numeric_origins(json: &Value, dataforge: &[u8]) -> TestResult {
    let forge = DataForge::open(dataforge)?;
    let tree = forge.record(0).ok_or("root Record")?.tree()?;
    for (member, expected) in [
        ("min_size", 2.0),
        ("max_size", 4.0),
        ("transform/position/0", 1.0),
        ("transform/position/1", 2.0),
        ("transform/rotation/2", 6.0),
    ] {
        let pointer = format!("{PORT}/definition/{member}");
        let ordinal = dataforge_ordinal(json, &pointer, "conversion")?;
        let mut original = None;
        tree.root().visit_owned_values(|visit| {
            if visit.ordinal() == ordinal {
                original = match visit.value() {
                    DataForgeValue::UInt32(value) => Some(f64::from(*value)),
                    DataForgeValue::Single(value) => Some(f64::from(*value)),
                    _ => None,
                };
            }
        });
        assert_eq!(
            original,
            Some(expected),
            "{pointer} points to the consumed original number"
        );
        assert_eq!(
            json.pointer(&pointer).and_then(Value::as_f64),
            Some(expected),
            "{pointer} remains compatible"
        );
    }
    Ok(())
}

fn verify_duplicate_subtypes(json: &Value) -> TestResult {
    let first = format!("{PORT}/definition/types/0/sub_types/0");
    let second = format!("{PORT}/definition/types/0/sub_types/1");
    let first = dataforge_ordinal(json, &first, "payload")?;
    let second = dataforge_ordinal(json, &second, "payload")?;
    assert_ne!(
        first, second,
        "duplicate spelling retains unique original array slots"
    );
    Ok(())
}

fn dataforge_ordinal(json: &Value, pointer: &str, role: &str) -> Result<usize, Box<dyn Error>> {
    let sites = field_uses(json, pointer)?;
    let field = sites
        .iter()
        .find(|site| {
            site["role"] == role
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
        })
        .ok_or("original DataForge occurrence")?;
    Ok(field
        .pointer("/location/indices/1")
        .and_then(Value::as_str)
        .ok_or("source ordinal")?
        .parse()?)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    entity_schema,
};
use serde_json::Value;

use super::{TestResult, field_uses, origin, original_bytes, source_path, verify_emitted_fields};

const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Probe.xml";
const PATCH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/Evidence.xml";
const BASE: &str = concat!(
    "<Vehicle name='Probe'><Parts><Part id='main' name='main'><ItemPort/></Part></Parts>",
    "<Modifications><Modification name='Tuned' patchFile='Modifications/Evidence.xml'/>",
    "</Modifications></Vehicle>",
);
const PATCH_XML: &str =
    "<Modifications><Part id='main'><ItemPort flags='locked'/></Part></Modifications>";

#[test]
fn vehicle_path_receipts_keep_the_trimmed_request_and_applied_patch_sites() -> TestResult {
    let definition = format!("  {VEHICLE}  ");
    let root = TestEntity::new(1, "Root", "entities/root.xml").vehicle(&definition, "  Tuned  ");
    let database = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .build();
    let json = loadout(
        &database,
        archive().entry(TestEntry::stored(
            format!("Data/{PATCH}"),
            PATCH_XML.as_bytes(),
        )),
    )?;
    let base_pointer = "/entity/vehicle/path";
    let applied_patch = "/entity/vehicle/patch_path";
    let modification = "/entity/vehicle/modification";
    for (pointer, value, complete) in [
        (base_pointer, VEHICLE, true),
        (applied_patch, PATCH, false),
        (modification, "Tuned", true),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_str),
            Some(value),
            "{pointer} keeps its selected value"
        );
        assert_eq!(
            origin(&json, pointer)?["containsEncryptedSource"],
            true,
            "{pointer} depends on encrypted DataForge"
        );
        assert_eq!(
            origin(&json, pointer)?["sourceSetComplete"],
            complete,
            "{pointer} reports the captured source scope"
        );
    }
    assert!(
        original_texts(&json, base_pointer, &database, "conversion")?
            .contains(&definition.as_bytes()),
        "the selected base path retains the original untrimmed definition"
    );
    assert!(
        original_texts(&json, modification, &database, "conversion")?
            .contains(&b"  Tuned  ".as_slice()),
        "the modification retains the original untrimmed request"
    );
    assert!(
        original_texts(&json, applied_patch, &database, "conversion")?
            .contains(&definition.as_bytes()),
        "the applied patch follows the selected base path"
    );
    verify_patch(&json, applied_patch)?;
    verify_emitted_fields(&json)
}

fn verify_patch(json: &Value, pointer: &str) -> TestResult {
    let sites = field_uses(json, pointer)?;
    let xml_attributes: Vec<_> = sites
        .iter()
        .filter(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("textXmlAttribute")
        })
        .map(|site| original_bytes(BASE.as_bytes(), site))
        .collect::<Result<_, _>>()?;
    assert!(
        xml_attributes.contains(&b"patchFile='Modifications/Evidence.xml'".as_slice()),
        "the applied patch retains the selected original patchFile attribute"
    );
    assert!(
        xml_attributes.contains(&b"name='Tuned'".as_slice()),
        "the applied patch retains the selected original Modification name"
    );
    let metadata = sites
        .into_iter()
        .find(|site| {
            site.pointer("/location/field").and_then(Value::as_str) == Some("path")
                && source_path(json, site).is_ok_and(|path| path == format!("Data/{PATCH}"))
        })
        .ok_or("selected patch directory occurrence")?;
    assert_eq!(
        metadata["role"], "selector",
        "the patch uses its archive directory match"
    );
    Ok(())
}

#[test]
fn cached_vehicle_class_fallback_keeps_the_original_and_current_record_occurrences() -> TestResult {
    let root = TestEntity::new(1, "Root", "entities/root.xml").loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::reference("first", 2),
        TestLoadoutEntry::reference("cached", 3),
    ]));
    let first = TestEntity::new(2, "Probe", "entities/first.xml").vehicle("  missing.xml ", "");
    let cached = TestEntity::new(3, "pROBE", "entities/cached.xml").vehicle(" MISSING.XML  ", "");
    let database = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(first.record())
        .record(cached.record())
        .build();
    let json = loadout(&database, archive())?;
    let first = "/entity/ports/0/entity/vehicle/path";
    let cached = "/entity/ports/1/entity/vehicle/path";
    for pointer in [first, cached] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_str),
            Some(VEHICLE),
            "{pointer} uses the first class fallback's path spelling"
        );
        assert_eq!(
            origin(&json, pointer)?["sourceSetComplete"],
            false,
            "{pointer} has an earlier missing file candidate"
        );
        assert!(
            original_texts(&json, pointer, &database, "conversion")?
                .contains(&b"EntityClassDefinition.Probe".as_slice()),
            "{pointer} carries the original class fallback's Record name"
        );
    }
    let selectors = original_texts(&json, cached, &database, "selector")?;
    assert!(
        selectors.contains(&b" MISSING.XML  ".as_slice()),
        "the cached selection retains the current caller's definition"
    );
    assert!(
        selectors.contains(&b"EntityClassDefinition.pROBE".as_slice()),
        "the cached selection retains the current caller's Record name"
    );
    let first_selectors = original_texts(&json, first, &database, "selector")?;
    assert!(
        !first_selectors.contains(&b"EntityClassDefinition.pROBE".as_slice()),
        "the original selection precedes the later caller"
    );
    verify_emitted_fields(&json)
}

fn original_texts<'input>(
    json: &Value,
    pointer: &str,
    dataforge: &'input [u8],
    role: &str,
) -> Result<Vec<&'input [u8]>, Box<dyn Error>> {
    field_uses(json, pointer)?
        .into_iter()
        .filter(|site| {
            site["role"] == role
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
        })
        .map(|site| original_bytes(dataforge, site))
        .collect()
}

fn archive() -> ArchiveBuilder {
    ArchiveBuilder::new().entry(TestEntry::stored(
        format!("Data/{VEHICLE}"),
        BASE.as_bytes(),
    ))
}

fn loadout(database: &[u8], builder: ArchiveBuilder) -> Result<Value, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    builder
        .entry(TestEntry::zstd("Data\\Game2.dcb", database).encrypted())
        .write_to(&archive)?;
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "entity", "loadout"])
        .arg(&archive)
        .arg("Root")
        .output()?;
    assert!(
        result.status.success(),
        "synthetic vehicle loadout resolves: {result:?}"
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

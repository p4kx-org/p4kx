// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::cryxml::{self, Element, Xml};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity,
    TestEntry, TestPort, entity_schema,
};
use serde_json::Value;

use super::{TestResult, array, field_uses, origin, original_bytes, source_path};

const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Probe.xml";
const PATCH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/Evidence.xml";
const BASE: &str = concat!(
    "\u{feff}",
    r#"<Vehicle name="Probe"><Parts>
<Part id="changed" name="hardpoint"><ItemPort id="flagTarget" flags="invisible old base"/></Part>
<Part id="inherit" name="inherited"><ItemPort flags="invisible inherited base"/></Part>
<Part id="last" name="overridden"><ItemPort id="target" flags="invisible before Elem"/></Part>
<Part id="absence" name="absent"><ItemPort/></Part>
<Part id="default" name="defaulted"><ItemPort id="noValue" flags="invisible before default"/></Part>
<Part id="decoy" name="HARDPOINT"><ItemPort flags="invisible decoy"/></Part>
</Parts><Modifications><Modification name="Tuned" patchFile="Modifications/Evidence.xml">
<Elems><Elem idRef="target" name="flags" value="invisible losing Elem"/>
<Elem idRef="target" name="Flags" value="visible winning Elem"/>
<Elem idRef="noValue" name="flags"/></Elems></Modification></Modifications></Vehicle>"#,
);

#[test]
fn vehicle_visibility_uses_the_applied_flags_attribute_for_aliases_and_overrides() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let entity = TestEntity::new(1, "Root", "entities/root.xml")
        .vehicle(VEHICLE, "Tuned")
        .port(TestPort::new("inherited").flags("different DataForge flags"));
    let database = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let patch = patch();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database))
        .entry(TestEntry::stored(
            format!("Data/{VEHICLE}"),
            BASE.as_bytes(),
        ))
        .entry(TestEntry::zstd(format!("Data/{PATCH}"), patch.clone()).encrypted())
        .write_to(&archive)?;
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "entity", "loadout"])
        .arg(&archive)
        .arg("Root")
        .output()?;
    assert!(
        result.status.success(),
        "synthetic XML visibility resolves: {result:?}"
    );
    let json: Value = serde_json::from_slice(&result.stdout)?;
    verify_patch(&json, &patch)?;
    verify_base_and_last_elem(&json)?;
    verify_absence(&json)
}

fn patch() -> Vec<u8> {
    let part = |id: &str, flags: Option<&str>| {
        let mut port = TestElement::new("ItemPort");
        if let Some(value) = flags {
            port = port.attribute("FLAGS", value);
        }
        TestElement::new("Part").attribute("id", id).child(port)
    };
    TestElement::new("Modifications")
        .child(
            TestElement::new("ItemPort")
                .attribute("id", "flagTarget")
                .attribute("flags", "visible earlier patch alias")
                .attribute("FLAGS", "invisible encrypted patch"),
        )
        .child(part("inherit", None))
        .child(part("last", Some("invisible shadowed patch")))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn verify_patch(json: &Value, patch: &[u8]) -> TestResult {
    let Xml::CryXml(document) = cryxml::read(patch)? else {
        return Err("test patch must decode as CryXml".into());
    };
    for name in ["hardpoint", "HARDPOINT"] {
        let pointer = visibility(json, name, "invisible")?;
        let receipt = origin(json, &pointer)?;
        assert_eq!(
            receipt["containsEncryptedSource"], true,
            "{name} consumes encrypted patch flags"
        );
        assert_eq!(
            receipt["sourceSetComplete"], false,
            "the full vehicle selector route is still incomplete"
        );
        verify_patch_selector(json, &pointer)?;
        let site = flag_use(json, &pointer)?;
        assert_eq!(
            source_path(json, site)?,
            format!("Data/{PATCH}"),
            "{name} uses the first physical Part"
        );
        assert_eq!(
            site.pointer("/location/namespace").and_then(Value::as_str),
            Some("cryxmlAttribute"),
            "{name} keeps the binary attribute coordinates"
        );
        assert_eq!(
            site.pointer("/location/field").and_then(Value::as_str),
            Some("FLAGS"),
            "{name} keeps the actual later donor spelling"
        );
        let [node, attribute] = array(site, "/location/indices")? else {
            return Err("CryXml node and attribute table indices".into());
        };
        let index = |value: &Value| -> Result<u32, Box<dyn Error>> {
            Ok(value.as_str().ok_or("original coordinate")?.parse()?)
        };
        let original = find_attribute(document.root(), index(node)?, index(attribute)?)
            .ok_or("the original binary flags")?;
        assert_eq!(
            original, "invisible encrypted patch",
            "{name} excludes the equal-folded decoy"
        );
    }
    Ok(())
}

fn verify_patch_selector(json: &Value, pointer: &str) -> TestResult {
    let sites = field_uses(json, pointer)?;
    let selector = sites
        .into_iter()
        .find(|site| {
            site.get("role").and_then(Value::as_str) == Some("selector")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("patchFile")
        })
        .ok_or("original patch file selection")?;
    assert_eq!(
        source_path(json, selector)?,
        format!("Data/{VEHICLE}"),
        "the patchFile selector belongs to the base implementation"
    );
    assert_eq!(
        original_bytes(BASE.as_bytes(), selector)?,
        br#"patchFile="Modifications/Evidence.xml""#,
        "visibility carries the actual patchFile selection attribute"
    );
    Ok(())
}

fn find_attribute(element: Element<'_>, node: u32, attribute: u32) -> Option<&str> {
    if element.index() == node {
        return element.source_attributes().find_map(|source| {
            let cryxml::SourceAttributePosition::CryXml {
                attribute_index, ..
            } = source.position()
            else {
                return None;
            };
            (*attribute_index == attribute).then(|| source.value())
        });
    }
    element
        .children()
        .find_map(|child| find_attribute(child, node, attribute))
}

fn verify_base_and_last_elem(json: &Value) -> TestResult {
    let inherited = visibility(json, "inherited", "invisible")?;
    let original = flag_use(json, &inherited)?;
    assert_eq!(
        source_path(json, original)?,
        format!("Data/{VEHICLE}"),
        "omitted patch flags inherit the original base attribute"
    );
    assert_eq!(
        original_bytes(BASE.as_bytes(), original)?,
        br#"flags="invisible inherited base""#,
        "original inherited bytes"
    );
    let overridden = visibility(json, "overridden", "visible")?;
    let original = flag_use(json, &overridden)?;
    assert_eq!(
        source_path(json, original)?,
        format!("Data/{VEHICLE}"),
        "the winning Elem is authored in the base file"
    );
    assert_eq!(
        original_bytes(BASE.as_bytes(), original)?,
        br#"value="visible winning Elem""#,
        "the last successful Elem assignment wins"
    );
    let uses = field_uses(json, &overridden)?;
    for selector in [
        br#"idRef="target""#.as_slice(),
        br#"name="Flags""#.as_slice(),
        br#"id="target""#.as_slice(),
    ] {
        assert!(
            uses.iter().any(|site| site["role"] == "selector"
                && original_bytes(BASE.as_bytes(), site).ok() == Some(selector)),
            "original Elem match includes {selector:?}"
        );
    }
    assert_eq!(
        origin(json, &overridden)?["containsEncryptedSource"],
        Value::Null,
        "overwritten patch flags do not taint the effective Elem value"
    );
    Ok(())
}

fn verify_absence(json: &Value) -> TestResult {
    for (name, field, bytes) in [
        ("absent", "flags", b"<ItemPort/>".as_slice()),
        (
            "defaulted",
            "value",
            br#"<Elem idRef="noValue" name="flags"/>"#.as_slice(),
        ),
    ] {
        let pointer = visibility(json, name, "visible")?;
        let site = flag_use(json, &pointer)?;
        assert_eq!(
            site.pointer("/location/namespace").and_then(Value::as_str),
            Some("textXmlElement"),
            "{name} keeps the directly checked scope"
        );
        assert_eq!(
            site.pointer("/location/field").and_then(Value::as_str),
            Some(field),
            "{name} retains the actual absent attribute name"
        );
        assert_eq!(
            original_bytes(BASE.as_bytes(), site)?,
            bytes,
            "{name} selects the authored empty default"
        );
    }
    Ok(())
}

fn flag_use<'a>(json: &'a Value, pointer: &str) -> Result<&'a Value, Box<dyn Error>> {
    let sites = field_uses(json, pointer)?;
    let conversions: Vec<_> = sites
        .into_iter()
        .filter(|site| site["role"] == "conversion")
        .collect();
    let [site] = conversions.as_slice() else {
        return Err("one effective flag conversion".into());
    };
    Ok(site)
}

fn visibility(json: &Value, name: &str, expected: &str) -> Result<String, Box<dyn Error>> {
    let (ordinal, port) = array(json, "/entity/ports")?
        .iter()
        .enumerate()
        .find(|(_, port)| port["name"] == name)
        .ok_or_else(|| format!("missing port {name}"))?;
    assert_eq!(port["visibility"], expected, "resolved port {name}");
    Ok(format!("/entity/ports/{ordinal}/visibility"))
}

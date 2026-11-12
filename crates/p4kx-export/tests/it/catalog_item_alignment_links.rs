// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestPort,
};
use serde_json::{Value, json};

use super::{CHILD, Layout, OWNER, database};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::loadouts_support::{loadout, port};

#[test]
fn default_loadouts_join_actual_physical_ports_and_item_components_under_filtering() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Complete), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    let unfiltered: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    check_default(&unfiltered, "projected")?;

    let filter = Filter::new("*/align_owner");
    options.filter = Some(&filter);
    let filtered: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let [only] = proof::rows(&filtered, super::ITEMS)? else {
        return Err("only the filtered owner has a catalog configuration".into());
    };
    proof::present(only, "ownerGuid", &json!(common::guid(OWNER)), Some(true))?;
    check_default(&filtered, "notProjected")
}

fn check_default(catalog: &Value, attachment_state: &str) -> TestResult {
    let root = loadout(catalog, OWNER)?;
    let mount = port(proof::at(root, "/ports")?, "mount")?;
    assert_eq!(
        proof::at(mount, "/source")?,
        "port_default",
        "the authored default still selects the child"
    );
    assert_eq!(
        proof::at(mount, "/minSize")?,
        &json!(1),
        "the port keeps its original minimum"
    );
    assert_eq!(
        proof::at(mount, "/maxSize")?,
        &json!(2),
        "the port keeps its original maximum"
    );
    let port_link = proof::at(mount, "/sourceConstraints/portDefinition")?;
    assert_eq!(
        proof::at(port_link, "/state")?,
        "projected",
        "the owning Record is selected"
    );
    proof::present(port_link, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(port_link, "componentOrdinal", &json!(1), Some(true))?;
    proof::present(port_link, "portOrdinal", &json!(0), Some(true))?;
    let item = proof::at(mount, "/sourceConstraints/attachedItem")?;
    let guid = proof::present(item, "targetGuid", &json!(common::guid(CHILD)), Some(true))?;
    assert!(
        proof::uses(catalog, guid)?.iter().any(|usage| {
            usage.pointer("/location/indices/0").and_then(Value::as_str) == Some("0")
        }),
        "the actual reference selection comes from the parent Record"
    );
    assert_eq!(
        proof::at(item, "/definition/state")?,
        attachment_state,
        "the link reports catalog selection independently of resolution"
    );

    let nested = port(proof::at(mount, "/ports")?, "nested")?;
    let nested_link = proof::at(nested, "/sourceConstraints/portDefinition")?;
    if attachment_state == "projected" {
        proof::present(nested_link, "ownerRecordIndex", &json!(1), Some(true))?;
        proof::present(nested_link, "portOrdinal", &json!(0), Some(true))?;
    } else {
        assert_eq!(
            proof::at(nested_link, "/state")?,
            "notProjected",
            "the child is resolved outside the filter"
        );
    }
    proof::closure(catalog)
}

#[test]
fn xml_selection_uses_the_first_named_definition_and_preserves_the_actual_xml_contribution()
-> TestResult {
    let catalog = xml_catalog()?;
    let [owner_ports] = proof::rows(&catalog, super::PORTS)? else {
        return Err("one physical port container".into());
    };
    let [first, duplicate] = proof::rows(owner_ports, "/ports/value")? else {
        return Err("both original same-name port definitions".into());
    };
    proof::present(
        proof::at(first, "/value")?,
        "name",
        &json!("mount"),
        Some(false),
    )?;
    proof::present(
        proof::at(duplicate, "/value")?,
        "name",
        &json!("MOUNT"),
        Some(false),
    )?;

    let root = loadout(&catalog, OWNER)?;
    let mount = port(proof::at(root, "/ports")?, "mount")?;
    assert_eq!(proof::at(mount, "/source")?, "loadout");
    assert_eq!(
        proof::at(mount, "/minSize")?,
        &json!(1),
        "the port keeps its original minimum"
    );
    assert_eq!(
        proof::at(mount, "/maxSize")?,
        &json!(2),
        "the port keeps its original maximum"
    );
    let port_link = proof::at(mount, "/sourceConstraints/portDefinition")?;
    proof::present(port_link, "portOrdinal", &json!(0), Some(false))?;
    let selected = proof::at(mount, "/sourceConstraints/attachedItem")?;
    let target = proof::present(
        selected,
        "targetGuid",
        &json!(common::guid(CHILD)),
        Some(true),
    )?;
    assert!(
        proof::uses(&catalog, target)?.iter().any(|usage| {
            usage.pointer("/location/namespace").and_then(Value::as_str) == Some("xmlAttribute")
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("@itemName")
        }),
        "the attached target keeps the original encrypted XML attribute selector"
    );
    let item_link = proof::at(selected, "/definition")?;
    proof::present(item_link, "ownerRecordIndex", &json!(1), Some(false))?;

    let unresolved = port(proof::at(root, "/ports")?, "lost")?;
    assert_eq!(proof::at(unresolved, "/unresolved")?, "unknown_class");
    assert_eq!(
        proof::at(unresolved, "/sourceConstraints/attachedItem")?,
        &Value::Null
    );
    proof::closure(&catalog)
}

fn xml_catalog() -> TestResult<Value> {
    const LOADOUT: &str = "Data/Synthetic/alignment.xml";
    const XML: &[u8] = br#"<Loadout><Items><Item portName="mount" itemName="FixtureXmlChild"/><Item portName="lost" itemName="FixtureXmlMissing"/></Items></Loadout>"#;
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(
                OWNER,
                "FixtureXmlOwner",
                "libs/foundry/records/entities/scitem/fixture/xml_owner.xml",
            )
            .attach_type("PowerPlant")
            .port(TestPort::new("mount").sizes(1, 2))
            .port(TestPort::new("MOUNT").sizes(8, 9))
            .loadout(TestLoadout::Xml(LOADOUT.to_owned()))
            .record(),
        )
        .record(
            TestEntity::new(
                CHILD,
                "FixtureXmlChild",
                "libs/foundry/records/entities/scitem/fixture/xml_child.xml",
            )
            .attach_type("PowerPlant")
            .record(),
        );
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", builder.build()))
        .entry(TestEntry::stored(LOADOUT, XML).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

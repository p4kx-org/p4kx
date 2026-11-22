// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::field_origin::{LITERAL, main_system, normal_catalog, source, system, target};
use super::{ALIAS_TARGET, ROOT};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const PLANET: u128 = 0x4101;
const MARKER: u128 = 0x4102;
const UNUSED: u128 = 0x4103;
const DETACHED: u128 = 0x4104;
const MINING: &str = "@MiningClaims_Desc_001";
const REACHED: &str = "Data/ObjectContainers/PU/Synthetic/Reached.socpak";
const FALLBACK: &str = "Data/ObjectContainers/PU/system/FallBack/fallback.socpak";

#[test]
fn the_marker_chain_identifies_the_actual_admitted_parent() -> TestResult {
    let fixture = with_parent(&branch(PLANET, MARKER, "1", true))?;
    let catalog = normal_catalog(&fixture)?;
    let row = only_row(&catalog)?;
    proof::present(row, "container", &json!(ALIAS_TARGET), Some(false))?;
    let parent = proof::present(
        row,
        "starmapObjectGuid",
        &json!(fixture::guid(PLANET)),
        Some(true),
    )?;
    let uses = proof::uses(&catalog, parent)?;
    assert_eq!(
        record_indices(&uses, "dataforgeRecordHeader", "guid")?,
        BTreeSet::from([0, 1]),
        "membership comes from the actual planet and marker headers"
    );
    assert_eq!(
        record_indices(&uses, "dataforgeValue", "$value")?,
        BTreeSet::from([0, 1]),
        "the exact description predicates distinguish marker and final non-marker"
    );
    assert!(
        uses.iter().any(
            |used| used.pointer("/location/field") == Some(&json!("@starMapRecord"))
                && used["role"] == "conversion"
        ),
        "the original Child route supplies the chosen GUID"
    );
    proof::closure(&catalog)
}

#[test]
fn ambiguous_marker_placements_keep_a_null_parent_and_both_routes() -> TestResult {
    let root = format!(
        "{}{}",
        branch(PLANET, MARKER, "1", true),
        branch(UNUSED, MARKER, "100", false),
    );
    let fixture = with_parent(&root)?;
    let (_, catalog) = fixture.diagnostic()?;
    let parent = proof::cell(only_row(&catalog)?, "starmapObjectGuid", "invalid")?;
    assert_eq!(
        parent["sourceSetComplete"], true,
        "both original choices prove the ambiguity"
    );
    let uses = proof::uses(&catalog, parent)?;
    let competing_positions = uses
        .iter()
        .filter(|used| {
            used.pointer("/location/field") == Some(&json!("@pos")) && used["role"] == "conversion"
        })
        .count();
    assert!(
        competing_positions >= 2,
        "physical placement comparison retains both candidates"
    );
    assert_eq!(
        record_indices(&uses, "dataforgeRecordHeader", "guid")?,
        BTreeSet::from([1]),
        "neither candidate parent GUID supplies the emitted null value"
    );
    proof::closure(&catalog)
}

#[test]
fn explicit_root_and_fallback_parent_absence_have_different_evidence() -> TestResult {
    let fixture = source(
        [
            main_system(LITERAL),
            system(FALLBACK, "fallback.xml", LITERAL),
        ],
        [target()],
    )?;
    let catalog = normal_catalog(&fixture)?;
    let [fallback, root] = proof::rows(&catalog, "/lootContainers")? else {
        return Err("both physical system occurrences must publish".into());
    };
    let absent = proof::cell(root, "starmapObjectGuid", "absent")?;
    let unknown = proof::cell(fallback, "starmapObjectGuid", "absent")?;
    assert_eq!(
        absent["sourceSetComplete"], true,
        "the chosen main root has no Child parent"
    );
    assert_eq!(
        unknown["sourceSetComplete"], false,
        "fallback does not attest every world route"
    );
    proof::closure(&catalog)
}

#[test]
fn an_unadmitted_parent_guid_does_not_certify_source_absence() -> TestResult {
    let fixture = with_parent(&branch(DETACHED, DETACHED, "1", true))?;
    let (_, catalog) = fixture.diagnostic()?;
    let parent = proof::cell(only_row(&catalog)?, "starmapObjectGuid", "absent")?;
    assert_eq!(
        parent["sourceSetComplete"], false,
        "the authored GUID was outside the admitted set"
    );
    assert!(
        record_indices(
            &proof::uses(&catalog, parent)?,
            "dataforgeRecordHeader",
            "guid"
        )?
        .is_empty(),
        "no unrelated DataForge header can stand in for the unresolved original GUID"
    );
    proof::closure(&catalog)
}

fn only_row(catalog: &Value) -> TestResult<&Value> {
    let [row] = proof::rows(catalog, "/lootContainers")? else {
        return Err("one configured placement under the original seed".into());
    };
    Ok(row)
}

fn record_indices(uses: &[&Value], namespace: &str, field: &str) -> TestResult<BTreeSet<u64>> {
    uses.iter()
        .filter(|used| {
            used["role"] == "selector"
                && used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
        })
        .map(|used| {
            let index = proof::at(used, "/location/indices/0")?
                .as_str()
                .ok_or("original Record index is a decimal string")?;
            Ok(index.parse()?)
        })
        .collect()
}

fn branch(outer: u128, marker: u128, position: &str, add_target: bool) -> String {
    let child = if add_target {
        format!(r#"<Child name="{REACHED}" external="1" pos="1,0,0"/>"#)
    } else {
        String::new()
    };
    let outer = fixture::guid(outer);
    let marker = fixture::guid(marker);
    format!(
        r#"<Child starMapRecord="{outer}" pos="0,0,0"><ChildObjectContainers>
        <Child starMapRecord="{marker}" pos="{position},0,0"><ChildObjectContainers>
        {child}
        </ChildObjectContainers></Child></ChildObjectContainers></Child>"#
    )
}

fn with_parent(children: &str) -> TestResult<Fixture> {
    let xml = format!(
        r#"<ObjectContainer version="14"><ChildObjectContainers>{children}</ChildObjectContainers></ObjectContainer>"#
    );
    let root = ArchiveBuilder::v2().entry(TestEntry::stored("syntheticsystem.xml", xml));
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject").field("description", TestType::Locale));
    for (guid, name, description) in [
        (PLANET, "Planet", "@Other"),
        (MARKER, "Mine", MINING),
        (UNUSED, "Unused", "@Unused"),
    ] {
        database = database.record(
            TestRecord::new(
                guid,
                "StarMapObject",
                format!("StarMapObject.{name}"),
                format!("libs/foundry/records/starmap/pu/system/synthetic/{name}.xml"),
            )
            .value("description", TestValue::text(description)),
        );
    }
    Fixture::open(
        &database.build(),
        Input::Encrypted,
        vec![
            TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>"),
            TestEntry::stored(ROOT, root.build()),
            system(REACHED, "reached.xml", LITERAL),
            target(),
        ],
    )
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use super::{EXPORTED_AT, TestResult, build, placement};
use crate::commodity_v3_fixture::{at, guid};
use crate::health_origin_assertions as proof;
use crate::starmap_fixture::{DOUBLE, JUMP};

const LEAF: u128 = 0x844_0001;
const PARENT: u128 = 0x844_0002;
const ROOT_XML: &str = "canarysystem.xml";
const TILE_XML: &str = "tile.xml";
const ROOT: &str = "Data/objectcontainers/pu/system/canary/canarysystem.socpak";
const TILE: &str = "ObjectContainers/PU/Starchitect/Tile/Tile.socpak";

#[test]
fn the_unselected_encrypted_ancestor_contributes_its_real_transform_and_route() -> TestResult {
    let root = format!(
        r#"<ObjectContainer version="14"><ChildObjectContainers>
        <Child external="1" name="{TILE}" entityName="tile-parent"
               starMapRecord="{}" pos="10,20,30" rot="0.7071067811865475,0,0,0.7071067811865475"/>
        </ChildObjectContainers></ObjectContainer>"#,
        guid(PARENT),
    );
    let tile = format!(
        r#"<ObjectContainer version="14"><ChildObjectContainers>
        <Child entityName="site" starMapRecord="{}" PoS="2,0,0" rot="1,0,0,0"/>
        </ChildObjectContainers></ObjectContainer>"#,
        guid(LEAF),
    );
    let catalog = catalog(root, tile, SourceArchive::EncryptedAncestor)?;
    assert_eq!(
        proof::rows(&catalog, "/definitions/starmapObjects/rows")?.len(),
        1
    );
    let leaf = placement(&catalog, LEAF)?;
    let position = proof::present(leaf, "position", &json!([10.0, 22.0, 30.0]), Some(true))?;
    let system = proof::present(leaf, "system", &json!("canary"), Some(true))?;
    let name = proof::present(leaf, "entityName", &json!("site"), Some(true))?;
    let parent = proof::present(leaf, "placedUnderGuid", &json!(guid(PARENT)), Some(true))?;
    assert!(has(&catalog, position, ROOT_XML, "@pos")?);
    assert!(has(&catalog, position, ROOT_XML, "@rot")?);
    assert!(has(&catalog, position, ROOT_XML, "@name")?);
    assert!(has(&catalog, position, TILE_XML, "@PoS")?);
    assert!(!has(&catalog, position, TILE_XML, "@rot")?);
    assert!(!has(&catalog, system, ROOT_XML, "@pos")?);
    assert!(has(&catalog, name, TILE_XML, "@entityName")?);
    assert!(!has(&catalog, name, ROOT_XML, "@entityName")?);
    assert!(has(&catalog, parent, ROOT_XML, "@starMapRecord")?);
    proof::closure(&catalog)
}

#[test]
fn an_unread_encrypted_external_target_is_not_an_actual_placement_contributor() -> TestResult {
    let root = format!(
        r#"<ObjectContainer version="14"><ChildObjectContainers>
        <Child external="1" name="{TILE}" starMapRecord="{}"/>
        </ChildObjectContainers></ObjectContainer>"#,
        guid(LEAF),
    );
    let catalog = catalog(
        root,
        "<ObjectContainer/>".to_owned(),
        SourceArchive::LockedTarget,
    )?;
    let leaf = placement(&catalog, LEAF)?;
    let position = proof::present(leaf, "position", &json!([0.0, 0.0, 0.0]), Some(false))?;
    let name = proof::cell(leaf, "entityName", "absent")?;
    assert_eq!(
        at(name, "/sourceSetComplete")?,
        true,
        "leaf absence is observed"
    );
    assert!(has(&catalog, name, ROOT_XML, "$absent:@entityName")?);
    assert!(has(&catalog, position, ROOT_XML, "$absent:@pos")?);
    assert!(!has(&catalog, position, ROOT_XML, "@name")?);
    for used in proof::uses(&catalog, position)? {
        assert_ne!(
            proof::source(&catalog, used)?.pointer("/path/value"),
            Some(&json!(TILE_XML))
        );
        assert_ne!(used.get("effectiveEncryption"), Some(&json!("encrypted")));
    }
    proof::closure(&catalog)
}

#[test]
fn ambiguity_uses_the_two_decisive_placements_and_a_repeat_keeps_its_selected_route() -> TestResult
{
    let (_, catalog) = build(None)?;
    let double = placement(&catalog, DOUBLE)?;
    let position = proof::cell(double, "position", "absent")?;
    assert_eq!(at(position, "/sourceSetComplete")?, true);
    assert!(has(&catalog, position, "stantonsystem.xml", "@pos")?);
    assert!(has(&catalog, position, "terrasystem.xml", "@pos")?);
    let jump = placement(&catalog, JUMP)?;
    let position = proof::present(jump, "position", &json!([0.0, 0.0, 50.0]), None)?;
    assert!(has(&catalog, position, "stantonsystem.xml", "@pos")?);
    assert!(!has(&catalog, position, "terrasystem.xml", "@pos")?);
    proof::closure(&catalog)
}

#[derive(Clone, Copy)]
enum SourceArchive {
    EncryptedAncestor,
    LockedTarget,
}

fn has(
    catalog: &Value,
    cell: &Value,
    path: &str,
    field: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    for used in proof::uses(catalog, cell)? {
        let source = proof::source(catalog, used)?;
        if source.pointer("/path/value") == Some(&json!(path))
            && at(used, "/location/field")? == field
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn catalog(
    root: String,
    tile: String,
    source: SourceArchive,
) -> Result<Value, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let tile = TestEntry::stored(format!("Data/{TILE}"), socpak(TILE_XML, tile));
    let (root, tile, key) = match source {
        SourceArchive::EncryptedAncestor => (
            TestEntry::stored(ROOT, socpak(ROOT_XML, root)).encrypted(),
            tile,
            Some(p4kx_testkit::test_key()),
        ),
        SourceArchive::LockedTarget => {
            let plain = ArchiveBuilder::v2()
                .entry(TestEntry::stored(ROOT_XML, root.into_bytes()).with_encryption_field(0))
                .entry(TestEntry::stored("empty.soc", Vec::new()))
                .build();
            (TestEntry::stored(ROOT, plain), tile.encrypted(), None)
        }
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge()))
        .entry(root)
        .entry(tile)
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, key)?;
    let filter = Filter::new("*/leaf*");
    let mut options = CatalogOptions::new(std::time::UNIX_EPOCH + EXPORTED_AT);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = Some(&filter);
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn dataforge() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject").field("hideInStarmap", TestType::Bool))
        .record(
            TestRecord::new(
                LEAF,
                "StarMapObject",
                "StarMapObject.Leaf",
                "libs/foundry/records/starmap/pu/system/canary/leaf.xml",
            )
            .value("hideInStarmap", TestValue::Bool(false)),
        )
        .record(
            TestRecord::new(
                PARENT,
                "StarMapObject",
                "StarMapObject.Parent",
                "libs/foundry/records/starmap/pu/system/canary/parent.xml",
            )
            .value("hideInStarmap", TestValue::Bool(false)),
        )
        .build()
}

fn socpak(name: &str, xml: String) -> Vec<u8> {
    TestContainer::new()
        .entry(TestEntry::stored(name, xml.into_bytes()))
        .entry(TestEntry::stored("empty.soc", Vec::new()))
        .build()
}

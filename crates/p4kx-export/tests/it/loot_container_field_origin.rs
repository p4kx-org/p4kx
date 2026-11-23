// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, build_catalog_from, catalog_schema};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestContainer, TestEntry};
use serde_json::{Value, json};

use super::{ALIAS_TARGET, ENTITY_DOCUMENT, ROOT, TARGET};
use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

#[path = "loot_container_tag_count_origin.rs"]
mod tag_count_origin;

pub(super) const CONFIG: &str = "687af3db-4c0d-4e8d-9261-102274f8b6ed";
const ALIAS: &str = "R:/authoring/Data/ObjectContainers/PU/synthetic/alias-target.socpak";
pub(super) const LITERAL: &str = "ObjectContainers/PU/synthetic/alias-target.socpak";
const COMPETING: &str = "Data/ObjectContainers/PU/system/synthetic/competing.socpak";

#[test]
fn the_first_physical_loot_origin_survives_a_competing_direct_link() -> TestResult {
    let source = source(
        [
            main_system(ALIAS).encrypted(),
            system(COMPETING, "competing.xml", LITERAL),
        ],
        [target()],
    )?;
    let catalog = normal_catalog(&source)?;
    let row = only_row(&catalog)?;
    let config = proof::present(row, "multiConfigGuid", &json!(CONFIG), Some(true))?;
    let path = proof::present(row, "container", &json!(ALIAS_TARGET), Some(true))?;
    verify_attribute(&catalog, config, "@multiConfigRef")?;
    verify_attribute(&catalog, path, "@objectContainer")?;
    let origins = proof::uses(&catalog, path)?;
    assert!(
        origins.iter().any(|used| {
            used.pointer("/location/field") == Some(&json!("@objectContainer"))
                && used["role"] == "conversion"
        }),
        "the winning repaired path retains the conversion actually used"
    );
    verify_target_path(&catalog, path)?;
    proof::closure(&catalog)
}

#[test]
fn a_direct_loot_link_keeps_its_own_clear_source() -> TestResult {
    let source = source(
        [
            main_system(LITERAL),
            system(COMPETING, "competing.xml", ALIAS).encrypted(),
        ],
        [target()],
    )?;
    let catalog = normal_catalog(&source)?;
    let row = only_row(&catalog)?;
    let config = proof::present(row, "multiConfigGuid", &json!(CONFIG), Some(false))?;
    let path = proof::present(row, "container", &json!(ALIAS_TARGET), Some(false))?;
    verify_attribute(&catalog, config, "@multiConfigRef")?;
    verify_attribute(&catalog, path, "@objectContainer")?;
    assert!(
        proof::uses(&catalog, path)?
            .iter()
            .all(
                |used| used.pointer("/location/field") != Some(&json!("@objectContainer"))
                    || used["role"] != "conversion"
            ),
        "only the discarded duplicate required authoring-path repair"
    );
    verify_target_path(&catalog, path)?;
    let count = proof::present(row, "entityCount", &json!(0), Some(false))?;
    proof::present(row, "tagCounts", &json!([]), Some(false))?;
    let uses = proof::uses(&catalog, count)?;
    assert!(
        uses.iter().any(|used| used.pointer("/location/namespace")
            == Some(&json!("socArchiveDirectory"))
            && used.pointer("/location/indices") == Some(&json!(["0", "1"]))
            && used["role"] == "selector"),
        "the count includes the exact selected container directory"
    );
    proof::closure(&catalog)
}

#[test]
fn a_verified_absent_loot_target_keeps_the_authored_path() -> TestResult {
    let source = source([main_system(TARGET).encrypted()], [])?;
    let catalog = normal_catalog(&source)?;
    let row = only_row(&catalog)?;
    let path = proof::present(row, "container", &json!(TARGET), Some(true))?;
    verify_attribute(&catalog, path, "@objectContainer")?;
    let count = proof::cell(row, "entityCount", "absent")?;
    proof::cell(row, "tagCounts", "absent")?;
    verify_attribute(&catalog, count, "@objectContainer")?;
    assert_eq!(
        proof::at(
            &catalog,
            "/declarations/lootEntityContainerDocuments/rows/0/links/value/0/targetStatus/value"
        )?,
        &json!("notFound"),
        "the original lookup proved that the target does not exist"
    );
    verify_fallback(&catalog, path)?;
    proof::closure(&catalog)
}

#[test]
fn ambiguous_loot_targets_keep_the_authored_path_without_a_candidate() -> TestResult {
    let source = source([main_system(LITERAL).encrypted()], [target(), target()])?;
    let (_, catalog) = source.diagnostic()?;
    let row = only_row(&catalog)?;
    let path = proof::present(row, "container", &json!(LITERAL), Some(true))?;
    verify_attribute(&catalog, path, "@objectContainer")?;
    verify_fallback(&catalog, path)?;
    let count = proof::cell(row, "entityCount", "unreadable")?;
    assert_eq!(
        count.get("sourceSetComplete"),
        Some(&json!(false)),
        "ambiguous targets do not prove a complete count",
    );
    assert!(
        matches!(
            build_catalog_from(&source.archive, &source.entry, &fixture::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "ambiguous target remains a refusal in Normal"
    );
    proof::closure(&catalog)
}

#[test]
fn a_normal_count_reports_the_actual_encrypted_entity_xml() -> TestResult {
    let source = source([main_system(LITERAL)], [target_with_soc(false)])?;
    let catalog = normal_catalog(&source)?;
    let count = proof::present(only_row(&catalog)?, "entityCount", &json!(2), Some(true))?;
    let uses = proof::uses(&catalog, count)?;
    let payloads: Vec<_> = uses
        .into_iter()
        .filter(|used| used.pointer("/location/namespace") == Some(&json!("socEntityChunk")))
        .collect();
    let [payload] = payloads.as_slice() else {
        return Err("one verified original entity XML range".into());
    };
    assert_eq!(
        proof::source(&catalog, payload)?.pointer("/path/value"),
        Some(&json!("entities.soc")),
        "the entity count cites the exact selected SOC member"
    );
    assert_eq!(
        payload.get("role"),
        Some(&json!("payload")),
        "the counted XML is a payload input"
    );
    proof::closure(&catalog)
}

#[test]
fn a_bad_soc_checksum_refuses_normal_and_has_no_count_payload() -> TestResult {
    let source = source([main_system(LITERAL)], [target_with_soc(true)])?;
    assert!(
        matches!(
            build_catalog_from(&source.archive, &source.entry, &fixture::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "Normal refuses a damaged SOC required by the placement"
    );
    let (_, catalog) = source.diagnostic()?;
    let row = only_row(&catalog)?;
    let count = proof::cell(row, "entityCount", "unreadable")?;
    proof::cell(row, "tagCounts", "unreadable")?;
    assert_eq!(
        count.get("sourceSetComplete"),
        Some(&json!(false)),
        "the failed SOC leaves an incomplete count"
    );
    assert!(
        proof::uses(&catalog, count)?
            .iter()
            .all(|used| { used.pointer("/location/namespace") != Some(&json!("socEntityChunk")) }),
        "a member that fails CRC provides no entity contribution"
    );
    proof::closure(&catalog)
}

fn target_with_soc(damaged: bool) -> TestEntry {
    let bytes = TestChunkFile::crch()
        .chunk(
            0x0004,
            1,
            b"<Entities><Entity/><Entity/></Entities>".to_vec(),
        )
        .bytes();
    let member = TestEntry::stored("entities.soc", bytes);
    let member = if damaged {
        member.with_crc32(0)
    } else {
        member
    };
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "alias-target.xml",
            r#"<ObjectContainer version="14"/>"#,
        ))
        .entry(member);
    let outer = TestEntry::stored(ALIAS_TARGET, container.build());
    if damaged { outer } else { outer.encrypted() }
}

pub(super) fn source(
    systems: impl IntoIterator<Item = TestEntry>,
    targets: impl IntoIterator<Item = TestEntry>,
) -> TestResult<Fixture> {
    let mut members = vec![TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>")];
    members.extend(systems);
    members.extend(targets);
    entries::with_starmap(members)
}

pub(super) fn main_system(selector: &str) -> TestEntry {
    system(ROOT, "syntheticsystem.xml", selector)
}

pub(super) fn system(path: &str, descriptor: &str, selector: &str) -> TestEntry {
    let container = ArchiveBuilder::v2()
        .entry(TestEntry::stored(
            descriptor,
            r#"<ObjectContainer version="14"/>"#,
        ))
        .entry(
            TestEntry::stored(
                ENTITY_DOCUMENT,
                format!(r#"<Entity objectContainer="{selector}" multiConfigRef="{CONFIG}"/>"#),
            )
            .with_encryption_field(0),
        );
    TestEntry::stored(path, container.build())
}

pub(super) fn target() -> TestEntry {
    let container = TestContainer::new().entry(TestEntry::stored(
        "alias-target.xml",
        r#"<ObjectContainer version="14"/>"#,
    ));
    TestEntry::stored(ALIAS_TARGET, container.build())
}

pub(super) fn normal_catalog(source: &Fixture) -> TestResult<Value> {
    let built = build_catalog_from(&source.archive, &source.entry, &fixture::options())?;
    assert!(
        built.summary().export_health().is_complete(),
        "Normal projected every source"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert!(
        jsonschema::validator_for(&catalog_schema())?.is_valid(&catalog),
        "the sourced rows validate"
    );
    Ok(catalog)
}

fn only_row(catalog: &Value) -> TestResult<&Value> {
    let [row] = proof::rows(catalog, "/lootContainers")? else {
        return Err("one canonical placement row after dedup".into());
    };
    Ok(row)
}

fn verify_attribute(catalog: &Value, cell: &Value, attribute: &str) -> TestResult {
    let inventory = proof::rows(catalog, "/provenance/sources")?;
    let owner = inventory
        .iter()
        .find(|source| source.pointer("/path/value") == Some(&json!(ROOT)))
        .and_then(|source| source.get("id"))
        .ok_or("original system owner source")?;
    let field = json!(attribute);
    let mut matched = 0;
    for used in proof::uses(catalog, cell)? {
        if used.pointer("/location/field") != Some(&field) {
            continue;
        }
        let source = proof::source(catalog, used)?;
        assert!(
            proof::rows(source, "/ancestors")?.contains(owner),
            "every retained attribute use comes from the winning physical EntXML"
        );
        assert_eq!(
            source.pointer("/path/value"),
            Some(&json!(ENTITY_DOCUMENT)),
            "the retained attribute originates in the expected member"
        );
        matched += 1;
    }
    assert!(
        matched > 0,
        "the value links to the exact authored XML field"
    );
    Ok(())
}

fn verify_target_path(catalog: &Value, cell: &Value) -> TestResult {
    let mut payloads = 0;
    for used in proof::uses(catalog, cell)? {
        if used.pointer("/location/field") == Some(&json!("path")) && used["role"] == "payload" {
            let source = proof::source(catalog, used)?;
            assert_eq!(
                source.pointer("/path/value"),
                Some(&json!(ALIAS_TARGET)),
                "the case-preserving resolved path is read from the winning target metadata"
            );
            payloads += 1;
        }
    }
    assert_eq!(
        payloads, 1,
        "one actual target path contributes to the resolved path"
    );
    Ok(())
}

fn verify_fallback(catalog: &Value, cell: &Value) -> TestResult {
    let uses = proof::uses(catalog, cell)?;
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/type") == Some(&json!("archiveDirectoryPaths"))),
        "the authored fallback still cites the complete physical lookup"
    );
    assert!(
        uses.iter().all(
            |used| used.pointer("/location/field") != Some(&json!("path"))
                || used["role"] != "payload"
        ),
        "no unavailable target path contributes to the emitted authored value"
    );
    Ok(())
}

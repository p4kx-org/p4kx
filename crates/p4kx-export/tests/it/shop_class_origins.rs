// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry,
};
use serde_json::Value;

use super::{EXCHANGE, LOOSE, TestResult, build};
use crate::health_origin_assertions as health;
use crate::shops_fixture::{self, REGISTRY};
use crate::starmap_fixture;

const FIRST: &str = "Data\\ObjectContainers\\PU\\Shops\\commex\\tdd\\first.socpak";
const AGREES: &str = "Data\\ObjectContainers\\PU\\Shops\\commex\\tdd\\second.socpak";
const DISAGREES: &str = "Data\\ObjectContainers\\PU\\Shops\\commex\\other\\second.socpak";

fn container() -> Vec<u8> {
    let metadata = TestElement::new("EntityComponentShop")
        .child(TestElement::new("Entity").attribute("CryGUID", EXCHANGE))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    TestContainer::new()
        .entry(
            TestEntry::stored("metadata\\entitycomponentshop.xml", metadata)
                .with_encryption_field(0),
        )
        .build()
}

fn catalog(extra: Option<TestEntry>) -> Result<Value, Box<dyn std::error::Error>> {
    let (_, catalog) = build(
        move |dir| {
            let path = dir.join("Data.p4k");
            let mut archive = ArchiveBuilder::v2()
                .entry(
                    TestEntry::stored("Data\\Game2.dcb", starmap_fixture::database().build())
                        .with_encryption_field(0),
                )
                .entry(
                    TestEntry::stored(REGISTRY, shops_fixture::registry()).with_encryption_field(0),
                )
                .entry(TestEntry::stored(FIRST, container()).with_encryption_field(0));
            if let Some(extra) = extra {
                archive = archive.entry(extra);
            }
            archive.write_to(&path)?;
            Ok(path)
        },
        None,
    )?;
    Ok(catalog)
}

fn entity<'a>(catalog: &'a Value, guid: &str) -> Result<&'a Value, Box<dyn std::error::Error>> {
    health::rows(catalog, "/shops")?
        .iter()
        .find(|row| row.pointer("/entityGuid/value").and_then(Value::as_str) == Some(guid))
        .ok_or_else(|| "the fixture Shop is missing".into())
}

fn contributors(catalog: &Value, cell: &Value, count: usize) -> TestResult {
    let uses = health::uses(catalog, cell)?;
    let exact = uses
        .iter()
        .filter(|source| {
            source.pointer("/location/field").and_then(Value::as_str) == Some("@CryGUID")
                && source["scope"] == "payload"
                && source["role"] == "selector"
        })
        .count();
    let physical_paths = uses
        .iter()
        .filter(|source| {
            source.pointer("/location/field").and_then(Value::as_str) == Some("path")
                && source["scope"] == "archiveDirectoryMetadata"
                && source["role"] == "selector"
        })
        .count();
    assert_eq!(
        exact, count,
        "each original nested CryGUID declaration contributes"
    );
    assert_eq!(
        physical_paths, count,
        "each matching outer directory path contributes"
    );
    assert!(
        uses.iter()
            .any(|source| source.pointer("/location/field").and_then(Value::as_str) == Some("@ID")),
        "the classification is joined by the selected registry ID"
    );
    Ok(())
}

#[test]
fn agreeing_classifications_retain_every_physical_selector_and_the_encrypted_ancestor() -> TestResult
{
    for encrypted in [false, true] {
        let extra = encrypted.then(|| TestEntry::stored(AGREES, container()).encrypted());
        let catalog = catalog(extra)?;
        let row = entity(&catalog, EXCHANGE)?;
        for (field, expected) in [("kind", "commex"), ("brand", "tdd")] {
            let cell = health::present(
                row,
                field,
                &Value::from(expected),
                encrypted.then_some(true),
            )?;
            contributors(&catalog, cell, if encrypted { 2 } else { 1 })?;
        }
        for field in ["kind", "brand"] {
            let cell = health::cell(entity(&catalog, LOOSE)?, field, "absent")?;
            assert_eq!(
                cell["sourceSetComplete"], false,
                "the nested negative survey is not fully represented in the field's origin"
            );
            assert_eq!(
                cell["containsEncryptedSource"],
                Value::Null,
                "a partial negative-survey source set cannot certify unencrypted absence"
            );
            contributors(&catalog, cell, 0)?;
        }
        health::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn different_brands_conflict_for_the_pair_and_failed_reads_leave_the_survey_incomplete()
-> TestResult {
    let mixed = catalog(Some(TestEntry::stored(DISAGREES, container()).encrypted()))?;
    let row = entity(&mixed, EXCHANGE)?;
    for field in ["kind", "brand"] {
        let cell = health::cell(row, field, "invalid")?;
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "the conflicting nested declaration is encrypted"
        );
        contributors(&mixed, cell, 2)?;
    }
    let incomplete = catalog(Some(
        TestEntry::stored(DISAGREES, b"broken".to_vec()).encrypted(),
    ))?;
    for (field, value) in [("kind", "commex"), ("brand", "tdd")] {
        let cell = health::cell(entity(&incomplete, EXCHANGE)?, field, "present")?;
        assert_eq!(cell["value"], value, "the observed class remains available");
        assert_eq!(
            cell["containsEncryptedSource"],
            Value::Null,
            "the incomplete clear contribution set is unknown"
        );
        assert_eq!(
            cell["sourceSetComplete"], false,
            "a failed container cannot finish the survey"
        );
        contributors(&incomplete, cell, 1)?;
        let unresolved = health::cell(entity(&incomplete, LOOSE)?, field, "unreadable")?;
        assert_eq!(
            unresolved["containsEncryptedSource"],
            Value::Null,
            "the failed encrypted archive does not contribute a declaration"
        );
        contributors(&incomplete, unresolved, 0)?;
    }
    health::closure(&mixed)?;
    health::closure(&incomplete)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::json;

#[test]
fn equal_missing_paths_distinguish_authored_xml_tokens_from_the_fixed_registry_constant()
-> TestResult {
    let authored = format!("<Entity objectContainer=\"{}\"/>", fixture::REGISTRY);
    let source =
        entries::with_starmap(vec![entries::container("alpha", authored.as_bytes(), true)])?;
    let outer = entries::original(&source.archive, &entries::outer("alpha"))?;
    let container = source.archive.open_container(outer)?;
    let inner = entries::original(&container, entries::INNER)?;
    let bytes = entries::read(&container, inner)?;
    let xml = std::str::from_utf8(&bytes)?;
    let document = roxmltree::Document::parse(xml)?;
    assert_eq!(
        document.root_element().attribute("objectContainer"),
        Some(fixture::REGISTRY),
        "the original authored missing target token is real"
    );
    assert!(
        source
            .archive
            .entries()
            .iter()
            .all(|entry| entry.path() != fixture::REGISTRY),
        "no target Entry exists"
    );
    let (_, catalog) = source.diagnostic()?;
    let rows = entries::failure(&catalog, "lootContainers", "enumeration")?;
    let row = rows.first().ok_or("authored missing target failure")?;
    let path = check::present_text(&catalog, row, "path", &json!(fixture::REGISTRY), Some(true))?;
    let uses = check::uses(&catalog, path)?;
    assert!(
        uses.iter()
            .any(|used| used.get("scope") == Some(&json!("payload"))
                && used.get("role") == Some(&json!("payload"))
                && used
                    .pointer("/location/field")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|field| field.contains("objectContainer"))
                && check::source(&catalog, used).is_ok_and(
                    |source| source.pointer("/path/value") == Some(&json!(entries::INNER))
                )),
        "the exact original authored selector contributes without a ghost target Entry"
    );
    assert!(
        check::rows(&catalog, "/provenance/sources")?
            .iter()
            .all(|source| source.pointer("/path/value") != Some(&json!(fixture::REGISTRY))),
        "a missing path cannot construct original Entry authority"
    );
    fixed_registry()?;
    check::closure(&catalog)
}

fn fixed_registry() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject"))
        .record(TestRecord::new(
            0x5712,
            "StarMapObject",
            "StarMapObject.Health",
            "libs/foundry/records/starmap/pu/system/alpha/health.xml",
        ))
        .build();
    let source = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let forge = source.entry.open()?;
    assert_eq!(
        forge
            .record(0)
            .ok_or("original starmap Record")?
            .type_name()?,
        "StarMapObject",
        "a real original row makes the registry required"
    );
    let (built, catalog) = source.diagnostic()?;
    assert_eq!(
        built.summary().starmap().objects,
        1,
        "the actual starmap consumer requires the fixed registry"
    );
    let failures = entries::failure(&catalog, "shops", "shopRegistry")?;
    let row = failures.first().ok_or("required fixed registry absence")?;
    let path = check::present_text(
        &catalog,
        row,
        "path",
        &json!(fixture::REGISTRY),
        Some(false),
    )?;
    check::independent(path)?;
    check::independent(check::cell(row, "targetGuid", "absent")?)?;
    check::closure(&catalog)
}

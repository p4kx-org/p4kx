// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Entry};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::bounded_output::{LimitKind, Limits};
use super::{ReducedError, plan, prepare_shop_slice};
use crate::CatalogFile;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const REGISTRY: &str = "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml";
const INVENTORY: &str = "Data/Scripts/ShopInventories/shop.json";
const ITEM: &str = "00000000-0000-4000-8000-000000000003";

#[test]
fn plan_requires_the_complete_fixed_scope() -> TestResult {
    let scope = include_str!("../../../../docs/catalog-basetool-scope.md");
    let mut selected = section_paths(scope)?;
    selected.sort_unstable();
    assert_eq!(
        selected,
        plan::SELECTION,
        "the fixed plan exactly covers the approved dataset paths"
    );
    assert!(
        plan::complete_scope(&selected),
        "the agreed complete selection passes"
    );
    assert!(
        !plan::complete_scope(&plan::SHOP_SCOPE),
        "a Shop slice cannot publish as the whole profile"
    );
    assert_eq!(plan::TEXT_SCOPE, ["texts.publicNames", "texts.gameTexts"]);
    assert_eq!(plan::LANGUAGES, ["en", "de"]);
    assert_eq!(
        (plan::FORMAT, plan::FORMAT_VERSION),
        ("p4kx-basetool-catalog", 1)
    );
    Ok(())
}

fn section_paths(scope: &str) -> TestResult<Vec<&'static str>> {
    let mut selected = vec![
        "schemaVersion",
        "source",
        "exportHealth",
        "completeness",
        "texts",
        "configurations",
        "declarations",
        "definitions",
        "placements",
        "projections",
    ];
    for section in [
        ("Top-level data", ""),
        ("Configurations", "configurations."),
        ("Declarations", "declarations."),
        ("Definitions", "definitions."),
        ("Placements and projections", ""),
    ] {
        let heading = format!("## {}", section.0);
        let mut lines = scope.lines();
        lines
            .find(|line| *line == heading)
            .ok_or("documented dataset section")?;
        for row in lines.take_while(|line| !line.starts_with("## ")) {
            if let Some((field, _)) = row.strip_prefix("| `").and_then(|row| row.split_once('`')) {
                let path = format!("{}{field}", section.1);
                let canonical = plan::SELECTION
                    .iter()
                    .find(|known| **known == path)
                    .ok_or_else(|| format!("missing selected dataset {path}"))?;
                selected.push(*canonical);
            }
        }
    }
    Ok(selected)
}

#[test]
fn reduced_shop_slice_resolves_original_values_and_ignores_unselected_dataforge() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), true)?;
    let target = scratch.path().join("shops.json.zst");
    let (file, sizes) =
        prepare_shop_slice(&archive, scratch.path(), None, Limits::default(), || {
            CatalogFile::create(&target)
        })?;
    assert!(!target.exists(), "the checked output remains staged");
    file.commit()?;
    let packed = fs::read(&target)?;
    let decoded = zstd::stream::decode_all(packed.as_slice())?;
    assert_eq!(
        u64::try_from(packed.len())?,
        sizes.compressed,
        "stored bytes include the final frame"
    );
    assert_eq!(
        u64::try_from(decoded.len())?,
        sizes.decoded,
        "decoded bytes include the newline"
    );
    let json: Value = serde_json::from_slice(&decoded)?;
    assert_eq!(at(&json, "/shops/0/inventory/value")?, "shop");
    assert_eq!(
        at(
            &json,
            "/declarations/shopInventoryResolution/rows/0/status/value"
        )?,
        "resolved"
    );
    assert_eq!(
        at(&json, "/shops/0/key/value")?,
        at(
            &json,
            "/declarations/shopInventoryResolution/rows/0/shopKey/value"
        )?,
        "the canonical shop key joins both tables"
    );
    for omitted in ["/shops/0/body", "/shops/0/defaultValue", "/schemaVersion"] {
        assert!(
            json.pointer(omitted).is_none(),
            "{omitted} is outside this staged slice"
        );
    }
    compare_inventory_values(&json)
}

fn compare_inventory_values(json: &Value) -> TestResult {
    let document = at(json, "/definitions/shopInventories/rows/0/document/value")?;
    assert_eq!(at(document, "/shopId/value/text")?, "desk");
    let stock = at(document, "/inventory/value/0/value")?;
    assert_eq!(
        at(stock, "/id/value/alternatives/value/0/value/text")?,
        ITEM
    );
    assert_eq!(at(stock, "/buyPrice/value")?.as_f64(), Some(12.5));
    assert_eq!(at(stock, "/sellPrice/value")?.as_u64(), Some(0));
    assert_eq!(at(stock, "/currentInventory/value")?.as_u64(), Some(3));
    assert_eq!(at(stock, "/maxInventory/value")?.as_u64(), Some(10));
    assert_eq!(
        at(stock, "/buyPrice/containsEncryptedSource")?.as_bool(),
        Some(true)
    );
    assert!(
        stock.pointer("/buyPrice/sourceUsesRefs").is_none(),
        "the reduced field has only its marker"
    );
    assert!(
        document.pointer("/source").is_none(),
        "the resolved value has no source arena requirement"
    );
    Ok(())
}

fn at<'value>(json: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    json.pointer(pointer)
        .ok_or_else(|| format!("missing reduced fixture field {pointer}").into())
}

#[test]
fn unreadable_selected_inventory_refuses_even_to_create_the_output() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), false)?;
    let target = scratch.path().join("unreadable.json.zst");
    let created = Cell::new(false);
    let result = prepare_shop_slice(&archive, scratch.path(), None, Limits::default(), || {
        created.set(true);
        CatalogFile::create(&target)
    });
    let index = archive
        .entries()
        .iter()
        .find(|entry| {
            entry
                .path()
                .replace('\\', "/")
                .eq_ignore_ascii_case(INVENTORY)
        })
        .map(Entry::index)
        .ok_or("selected fixture inventory")?;
    assert!(
        matches!(result, Err(ReducedError::UnreadableInventory(actual)) if u32::try_from(index) == Ok(actual))
    );
    assert!(
        !created.get(),
        "the failing selected inventory never opens output"
    );
    Ok(())
}

#[test]
fn decoded_and_compressed_limits_preserve_the_previous_output() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path(), true)?;
    let target = scratch.path().join("bound.json.zst");
    fs::write(&target, b"retained")?;
    for (limits, expected, opens) in [
        (
            Limits {
                decoded: 1,
                ..Limits::default()
            },
            LimitKind::Decoded,
            false,
        ),
        (
            Limits {
                compressed: 1,
                ..Limits::default()
            },
            LimitKind::Compressed,
            true,
        ),
    ] {
        let created = Cell::new(false);
        let result = prepare_shop_slice(&archive, scratch.path(), None, limits, || {
            created.set(true);
            CatalogFile::create(&target)
        });
        assert!(matches!(result, Err(ReducedError::Limit { kind, limit: 1 }) if kind == expected));
        assert_eq!(
            created.get(),
            opens,
            "the decoded cap is checked before creating any file"
        );
        assert_eq!(
            fs::read(&target)?,
            b"retained",
            "the previously published output is preserved"
        );
    }
    Ok(())
}

fn fixture(path: &Path, with_key: bool) -> TestResult<Archive> {
    let output = path.join("Data.p4k");
    let registry = r#"<BackupData><LayerBackups><LayerBackup PlatformName="FixtureSystem"><ShopBackups>
        <ShopBackup ID="00000000-0000-4000-8000-000000000001,00000000-0000-4000-8000-000000000002" SuperName="Fixture,Shop"/>
    </ShopBackups></LayerBackup></LayerBackups></BackupData>"#;
    let prefab = r#"<PrefabsLibrary><Prefab><Objects><Object Type="Entity" Id="{00004000-0000-0000-0200-000000000080}">
        <PropertiesDataCore><EntityComponentShop offlineInventoryJSON="Scripts/ShopInventories/shop.json"/></PropertiesDataCore>
    </Object></Objects></Prefab></PrefabsLibrary>"#;
    let data = format!(
        r#"{{"ShopID":"desk","Inventory":[{{"ID":{{"ID":["{ITEM}"]}},"BuyPrice":12.5,"SellPrice":0,"CurrentInventory":3,"MaxInventory":10}}]}}"#
    );
    ArchiveBuilder::v2()
        .entry(TestEntry::stored(REGISTRY, registry).with_encryption_field(0))
        .entry(TestEntry::stored("Data/Prefabs/Shops/shop.xml", prefab).with_encryption_field(0))
        .entry(TestEntry::stored(INVENTORY, data).encrypted())
        .entry(
            TestEntry::stored("Data/Game2.dcb", "unselected invalid file").with_encryption_field(0),
        )
        .write_to(&output)?;
    Ok(Archive::open_with_key(output, with_key.then(test_key))?)
}

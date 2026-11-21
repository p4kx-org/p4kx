// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Record, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};
use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SOURCE: &str = "ContractScopeGlobalParams";
const KIND: Kind = Kind::ContractScopes;
const SLOTS: [&str; 7] = [
    "location",
    "cluster",
    "sector",
    "planetaryBody",
    "planetarySystem",
    "solarSystem",
    "universe",
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn contract_scopes_match_original_scope_items() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let originals = scope_records(&forge, &watch)?;
    if originals.is_empty() && std::env::var_os("P4KX_EXPECT_CONTRACT_SCOPES").is_some() {
        return Err("expected a physical contract scope Record".into());
    }
    for record in &originals {
        let filter = Filter::new(record.record_path().map_err(|_| "scope Record path")?);
        let catalog = catalog(&archive, &entry, &filter)?;
        let rows = consumer::rows(&catalog, KIND)?;
        let row = physical_row(rows, record)?;
        let tree = TreeBuilder::new()
            .build(record)
            .map_err(|_| "read original scope Record body")?;
        let texts = TextIndex::new(&catalog)?;
        for slot in SLOTS {
            compare_slot(tree.field(slot), field(row, slot)?, &texts)?;
        }
    }
    if originals.is_empty() {
        let filter = Filter::new("synthetic/scope-witness/no-source");
        let catalog = catalog(&archive, &entry, &filter)?;
        let rows = consumer::rows(&catalog, KIND)?;
        assert!(rows.is_empty(), "no physical scope owner exists");
        let declared = forge
            .structs()
            .any(|definition| definition.name() == Ok(SOURCE));
        let availability = if declared { "available" } else { "absent" };
        assert_eq!(
            catalog
                .pointer("/definitions/contractScopes/availability")
                .and_then(Value::as_str),
            Some(availability),
            "declaration availability remains independent of physical rows"
        );
        print_line("No physical contract scope Record exists in this Archive");
    } else {
        print_line("Physical contract scope records and authored slots matched");
    }
    watch
        .check()
        .map_err(|_| "Archive changed during scope verification")?;
    Ok(())
}

fn scope_records<'source>(
    forge: &'source DataForge<'_>,
    watch: &ArchiveWatch,
) -> TestResult<Vec<Record<'source>>> {
    let mut rows = Vec::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during scope census")?;
        if record.type_name().map_err(|_| "original Record type")? == SOURCE {
            rows.push(record);
        }
    }
    Ok(rows)
}

fn catalog(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> TestResult<Value> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build the source-selected scope catalog")?;
    let catalog = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn physical_row<'row>(rows: &'row [Value], record: &Record<'_>) -> TestResult<&'row Value> {
    let mut matching = rows.iter().filter(|row| {
        row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(record.index()))
    });
    let row = matching.next().ok_or("expected one physical scope row")?;
    assert!(
        matching.next().is_none(),
        "a physical scope owner has one row"
    );
    assert!(
        consumer::identity(row, KIND)? == record.guid().to_string(),
        "scope row uses the actual physical GUID"
    );
    Ok(row)
}

fn compare_slot(source: Option<&Original<'_>>, cell: &Value, texts: &TextIndex<'_>) -> TestResult {
    let Some(Original::Struct(item)) = source else {
        return verify_state(source, cell);
    };
    let row = consumer::present(cell)?;
    assert_eq!(
        consumer::present(field(row, "sourceType")?)?.as_str(),
        Some(item.type_name()),
        "original concrete scope item type"
    );
    compare_icon(item, row)?;
    compare_name(item, row, texts)
}

fn compare_icon(item: &StructValue<'_>, row: &Value) -> TestResult {
    let actual = field(row, "icon")?;
    let source = item.field("icon");
    let Some(Original::String(expected)) = source else {
        return verify_state(source, actual);
    };
    assert!(
        consumer::present(actual)?.as_str() == Some(*expected),
        "original scope icon is preserved"
    );
    Ok(())
}

fn compare_name(item: &StructValue<'_>, row: &Value, texts: &TextIndex<'_>) -> TestResult {
    let actual = field(row, "displayName")?;
    let source = item.field("displayName");
    let Some(Original::Locale(expected)) = source else {
        return verify_state(source, actual);
    };
    let name = texts
        .reference(actual, TextDomain::PublicName, TextReferenceShape::Required)?
        .ok_or("source scope display name")?;
    assert!(
        name.source().pointer("/key/value").and_then(Value::as_str) == Some(*expected),
        "the public name retains the exact original Locale key"
    );
    Ok(())
}

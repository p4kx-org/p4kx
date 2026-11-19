// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::BufReader;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, CatalogSummary, Compression, ExportStageStatus, Kind,
    SCHEMA_VERSION, write_catalog_from,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, Filter, InputScope, Record, RecordTree,
    SourceEncryption, SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::catalog_v3_consumer;
use crate::real_archive_catalog_pipe::{self as pipe, CountingReader};
use crate::real_archive_catalog_table_parity::collect_references;
use crate::real_archive_item_durability::Trace;
use crate::real_archive_shop_inventory_v3::verify_build;

#[path = "real_archive_cargo_globals_sdk.rs"]
mod sdk;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const CARGO: &str = "GlobalCargoLoadingParams";
const SHOP: &str = "GlobalShopCommodityParams";
const FILTER: &str = "libs/foundry/records/global*params/*";
const LOADING_SCALARS: &[&str] = &[
    "autoLoadingBaseLoadingTime",
    "autoLoadingBaseUnloadingTime",
    "cargoDeckLoadingTimePerSCU",
];
const LOADING_MAPS: &[(&str, &str)] = &[
    (
        "autoLoadingBoxSizeLoadingTime",
        "SAutoLoadingBoxSizeLoadingTime",
    ),
    (
        "autoLoadingBoxSizeUnloadingTime",
        "SAutoLoadingBoxSizeLoadingTime",
    ),
];
const PRICE_SCALARS: &[&str] = &["autoLoadingBasePrice"];
const PRICE_MAPS: &[(&str, &str)] = &[("autoLoadingBoxSizePrices", "SAutoLoadingBoxSizePrices")];
const BOX_SIZES: [(&str, &str); 10] = [
    ("0.125", "one_eighthSCU"),
    ("0.25", "one_quarterSCU"),
    ("0.5", "one_halfSCU"),
    ("1", "oneSCU"),
    ("2", "twoSCU"),
    ("4", "fourSCU"),
    ("8", "eightSCU"),
    ("16", "sixteenSCU"),
    ("24", "twentyFourSCU"),
    ("32", "thirtyTwoSCU"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCatalog {
    schema_version: u32,
    source: Value,
    configurations: WireCargo,
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WireCargo {
    cargo_loading_globals: Value,
    shop_commodity_globals: Value,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and optionally P4KX_DATA_P4K_B and pass --ignored"]
fn installed_cargo_globals_match_original_timings_prices_and_scu_selectors() -> TestResult {
    let primary = real_archive_path().map_err(|_| "configure the installed Archive")?;
    let key = real_archive_key().map_err(|_| "read the configured Archive key")?;
    for path in std::iter::once(primary).chain(real_archive_b_path()) {
        let archive = Archive::open_with_key(path, Some(key.clone()))
            .map_err(|_| "open the configured Archive")?;
        compare_archive(&archive)?;
    }
    Ok(())
}

fn compare_archive(archive: &Archive) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let entry = DataForgeEntry::read(archive).map_err(|_| "read the original DataForge")?;
    let forge = entry.open().map_err(|_| "open the original DataForge")?;
    let filter = Filter::parse(FILTER)?;
    let owners = scoped_originals(forge.records(), &filter)?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (summary, (wire, bytes)) = pipe::transfer(
        |reader| {
            let mut reader = BufReader::new(CountingReader::new(reader));
            let wire: WireCatalog = serde_json::from_reader(&mut reader)?;
            Ok::<_, serde_json::Error>((wire, reader.into_inner().byte_count()))
        },
        |sink| write_catalog_from(archive, &entry, &options, Compression::None, || Ok(sink)),
    )
    .map_err(|_| "project and decode the scoped cargo catalog")?;
    if bytes != summary.size() || wire.schema_version != SCHEMA_VERSION {
        return Err("the scoped v3 catalog must reconcile its output bytes".into());
    }
    let catalog = json!({"source": wire.source, "configurations": wire.configurations});
    verify_build(archive, &catalog)?;
    verify_selected_health(&summary)?;
    compare_rows(&entry, &owners, &catalog, &summary)?;
    sdk::verify(archive, &entry, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during the cargo comparison")?;
    Ok(())
}

fn scoped_originals<'source>(
    records: impl Iterator<Item = Record<'source>>,
    filter: &Filter,
) -> TestResult<Vec<Record<'source>>> {
    let owners: Vec<_> = records
        .filter(|record| matches!(record.type_name(), Ok(CARGO | SHOP)))
        .collect();
    for name in [CARGO, SHOP] {
        if !owners.iter().any(|record| record.type_name() == Ok(name)) {
            return Err(format!("the Build lacks the original {name} Record").into());
        }
    }
    for owner in &owners {
        let path = owner
            .record_path()
            .map_err(|_| "read the original owner path")?;
        if !filter.matches(path) {
            return Err("the scoped catalog Filter must include every original cargo owner".into());
        }
    }
    Ok(owners)
}

fn verify_selected_health(summary: &CatalogSummary) -> TestResult {
    for kind in [Kind::CargoLoadingGlobals, Kind::ShopCommodityGlobals] {
        let health = summary
            .export_health()
            .kind(kind)
            .ok_or("missing cargo kind health")?;
        if !health.source_inventory_complete
            || !health.field_inventory_complete
            || health.read != ExportStageStatus::Complete
            || health.projection != ExportStageStatus::Complete
        {
            return Err(format!("incomplete scoped source or projection for {kind}").into());
        }
    }
    Ok(())
}

fn compare_rows(
    entry: &DataForgeEntry,
    owners: &[Record<'_>],
    catalog: &Value,
    summary: &CatalogSummary,
) -> TestResult {
    let loading = catalog_v3_consumer::rows(catalog, Kind::CargoLoadingGlobals)?;
    let prices = catalog_v3_consumer::rows(catalog, Kind::ShopCommodityGlobals)?;
    if loading.len() + prices.len() != owners.len() {
        return Err("the two tables must preserve the physical cargo owner count".into());
    }
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("expected the original Archive DataForge input".into());
    };
    let source = SourceUses::new();
    let scope = source.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let encryption = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let referenced = referenced_uses(summary, loading.iter().chain(prices))?;
    let uses = referenced
        .iter()
        .map(|row| {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .ok_or("actual source-use ID")?;
            Ok((id, row))
        })
        .collect::<TestResult<HashMap<_, _>>>()?;
    for owner in owners {
        let tree = owner.tree().map_err(|_| "read the original cargo Record")?;
        let rows = if tree.type_name() == CARGO {
            loading
        } else {
            prices
        };
        let row = rows
            .iter()
            .find(|row| {
                row.pointer("/recordIndex/value").and_then(Value::as_u64)
                    == Some(u64::from(tree.record_index()))
            })
            .ok_or("expected the exact physical cargo Record row")?;
        let trace = Trace::of(&tree, &uses, &scope, encryption);
        compare_header(&trace, row, &tree)?;
        compare_record(&trace, tree.root(), row, tree.type_name())?;
    }
    Ok(())
}

fn compare_header(trace: &Trace<'_, '_>, row: &Value, tree: &RecordTree<'_>) -> TestResult {
    if row.pointer("/guid/value").and_then(Value::as_str) != Some(tree.guid().to_string().as_str())
        || row.pointer("/recordPath/value").and_then(Value::as_str) != Some(tree.record_path())
        || row.pointer("/sourceType/value").and_then(Value::as_str) != Some(tree.type_name())
    {
        return Err("catalog cargo identity differs from the original Record".into());
    }
    for (cell, field, role) in [
        ("guid", "guid", "payload"),
        ("recordIndex", "recordIndex", "selector"),
        ("recordPath", "fileName", "conversion"),
        ("sourceType", "sourceType", "selector"),
    ] {
        trace.header(
            row.get(cell).ok_or("required cargo header carrier")?,
            field,
            role,
        )?;
    }
    Ok(())
}

fn referenced_uses<'wire>(
    summary: &CatalogSummary,
    rows: impl Iterator<Item = &'wire Value>,
) -> TestResult<Vec<Value>> {
    let mut references = HashSet::new();
    for row in rows {
        collect_references(row, &mut references);
    }
    summary
        .source_inventory()
        .iter_uses()
        .filter(|row| references.contains(row.id().as_str()))
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

fn compare_record<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
    nominal: &str,
) -> TestResult {
    let (scalars, maps) = if nominal == CARGO {
        (LOADING_SCALARS, LOADING_MAPS)
    } else {
        (PRICE_SCALARS, PRICE_MAPS)
    };
    for field in scalars {
        compare_scalar(trace, original.field(field), row.get(field), field)?;
    }
    for &(field, nominal) in maps {
        compare_map(trace, original, row, field, nominal)?;
    }
    Ok(())
}

fn compare_map<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
    field: &str,
    nominal: &str,
) -> TestResult {
    let cell = row
        .get(field)
        .ok_or("expected the cargo size table carrier")?;
    let body = catalog_v3_consumer::present(cell)?;
    let Some(raw @ Original::Struct(source)) = original.field(field) else {
        return Err(format!("missing original {field} size table").into());
    };
    let source_type = body
        .get("sourceType")
        .ok_or("cargo size Class type carrier")?;
    if source.type_name() != nominal
        || catalog_v3_consumer::present(source_type)?.as_str() != Some(nominal)
        || body.get("additionalFields").is_none()
    {
        return Err(format!("wrong source type or missing additions for {field}").into());
    }
    trace.part(cell, raw, "$type", "selector")?;
    trace.part(source_type, raw, "$type", "selector")?;
    let sizes = body
        .get("byScu")
        .and_then(Value::as_object)
        .ok_or("expected SCU-keyed cargo sizes")?;
    if sizes.len() != BOX_SIZES.len() {
        return Err(format!("incorrect known SCU size count in {field}").into());
    }
    if !BOX_SIZES
        .iter()
        .any(|&(_, member)| source.field(member).is_some())
    {
        return Err(format!("no recognized size on original {field} declaration").into());
    }
    for (size, member) in BOX_SIZES {
        compare_scalar(trace, source.field(member), sizes.get(size), member)?;
    }
    Ok(())
}

fn compare_scalar<'source>(
    trace: &Trace<'_, 'source>,
    original: Option<&Original<'source>>,
    cell: Option<&Value>,
    field: &str,
) -> TestResult {
    let cell = cell.ok_or("expected an explicit original cargo scalar carrier")?;
    let payload = catalog_v3_consumer::source_value(cell)?;
    match (original, payload) {
        (Some(raw @ Original::Single(expected)), Some(value)) => {
            let bits = value.get("rawBits").and_then(Value::as_str);
            let finite =
                Option::<f32>::deserialize(value.get("finite").ok_or("timing finite view")?)?;
            if bits != Some(format!("{:08x}", expected.to_bits()).as_str())
                || finite.map(f32::to_bits) != expected.is_finite().then_some(expected.to_bits())
            {
                return Err(format!("original Float32 differs for {field}").into());
            }
            trace.check(cell, raw, "payload")
        }
        (Some(raw @ Original::Int32(expected)), Some(value)) => {
            if value.as_i64() != Some(i64::from(*expected)) {
                return Err(format!("original Int32 differs for {field}").into());
            }
            trace.check(cell, raw, "payload")
        }
        (Some(raw @ Original::Null), None)
            if cell.get("state").and_then(Value::as_str) == Some("null") =>
        {
            trace.check(cell, raw, "selector")
        }
        (None, None)
            if cell.get("state").and_then(Value::as_str) == Some("absent")
                && cell["sourceSetComplete"] == true
                && cell["sourceUsesRefs"].as_array().is_some_and(Vec::is_empty) =>
        {
            Ok(())
        }
        _ => Err(format!("original cargo scalar state differs for {field}").into()),
    }
}

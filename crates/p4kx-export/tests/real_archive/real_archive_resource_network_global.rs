// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::resource_network::with_resource_network_globals_from;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceUses, StructValue, Value,
};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value as Json;

use crate::component_sdk_parity::ComponentSdkParity;
use crate::real_archive_item_components::single;
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

const DATASET: &str = "/definitions/resourceNetworkGlobals/rows";
const SOURCE_TYPE: &str = "ItemResourceNetworkGlobal";
const ROOT_NUMBERS: &[&str] = &[
    "maxPowerToCoolantRatio",
    "thrusterCoolantMultiplier",
    "powerBaseConversionRate",
    "misfireRepairGracePeriod",
];
const DISTRIBUTION_NUMBERS: &[&str] = &[
    "powerDistCoolerMin",
    "powerDistPoolsProportion",
    "poolDefaultWeapons",
    "poolDefaultEngines",
    "poolDefaultShields",
];

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn emitted_global_power_settings_match_the_original_records() -> TestResult {
    let Some(pair) = real_archive_pair()? else {
        return Ok(());
    };
    let key = real_archive_key()?;
    let paths: [_; 2] = pair.into();
    for path in paths {
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        let watch = ArchiveWatch::new(&archive);
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let mut records = forge
            .records()
            .filter(|row| row.type_name().ok() == Some(SOURCE_TYPE));
        let record = records
            .next()
            .ok_or("physical global Resource Network Record")?;
        assert!(
            records.next().is_none(),
            "the selected build has one global Record"
        );
        let tree = record.tree()?;
        let count = verify_record(&archive, &entry, &tree)?;
        watch.check()?;
        if let Some(build) = archive.build() {
            print_line(&format!(
                "global power Build {}: {count} original modifiers",
                build.change_number()
            ));
        }
    }
    Ok(())
}

fn verify_record(
    archive: &Archive,
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
) -> TestResult<usize> {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, options.with_full_provenance())?;
    let catalog: Json = serde_json::from_slice(built.json())?;
    verify_build(archive, &catalog)?;
    let rows = proof::rows(&catalog, DATASET)?;
    let [row] = rows else {
        return Err("one filtered global resource definition".into());
    };
    assert_eq!(
        row.pointer("/recordIndex/value").and_then(Json::as_u64),
        Some(u64::from(tree.record_index())),
        "the exported global Record identifies its exact source position"
    );
    assert_eq!(
        row.pointer("/sourceType/value").and_then(Json::as_str),
        Some(SOURCE_TYPE),
        "the definition preserves the original Record type",
    );
    let comparison = ComponentSdkParity::new(archive, entry, &catalog)?;
    with_resource_network_globals_from(
        archive,
        entry,
        CatalogRecordSelection::RecordIndex(tree.record_index()),
        None,
        |snapshot| comparison.compare_rows(DATASET, &[row], &snapshot),
    )??;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("expected an observed Archive DataForge".into());
    };
    let uses = proof::indexed(&catalog, "/provenance/uses", "id")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let marker = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(tree, &uses, &scope, marker);
    verify_payloads(&trace, tree, row)
}

fn verify_payloads<'source>(
    trace: &Trace<'_, 'source>,
    tree: &RecordTree<'source>,
    row: &Json,
) -> TestResult<usize> {
    singles(trace, tree.root(), row, ROOT_NUMBERS)?;
    let (settings, exported) = owned(tree.root(), row, "defaultPowerDistributionParams")?;
    singles(trace, settings, exported, DISTRIBUTION_NUMBERS)?;
    let raw = settings
        .field("powerDistPoolsFillMin")
        .ok_or("original pool Boolean")?;
    let value = exported
        .get("powerDistPoolsFillMin")
        .ok_or("exported pool Boolean")?;
    if let Value::Bool(flag) = raw {
        assert_eq!(value["value"], *flag, "the original pool Boolean survives");
    } else {
        return Err("original pool fill control is a Boolean".into());
    }
    trace.check(value, raw, "payload")?;
    let (power, exported) = owned(tree.root(), row, "powerParams")?;
    modifiers(trace, power, exported)
}

fn owned<'tree, 'source, 'wire>(
    source: &'tree StructValue<'source>,
    exported: &'wire Json,
    field: &str,
) -> TestResult<(&'tree StructValue<'source>, &'wire Json)> {
    let Some(Value::Struct(body)) = source.field(field) else {
        return Err("expected original owned power body".into());
    };
    let value = exported
        .get(field)
        .and_then(|cell| cell.get("value"))
        .ok_or("exported power body")?;
    Ok((body, value))
}

fn singles<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    exported: &Json,
    names: &[&str],
) -> TestResult {
    for name in names {
        let value = exported.get(name).ok_or("exported original Single")?;
        single(source, name, value)?;
        trace.check(
            value,
            source.field(name).ok_or("original Single")?,
            "payload",
        )?;
    }
    Ok(())
}

fn modifiers<'source>(
    trace: &Trace<'_, 'source>,
    power: &StructValue<'source>,
    exported: &Json,
) -> TestResult<usize> {
    let Some(Value::Array(modifiers)) = power.field("modifiers") else {
        return Err("original modifier array".into());
    };
    let values = proof::rows(exported, "/modifiers/value")?;
    assert_eq!(
        values.len(),
        modifiers.len(),
        "every original modifier slot survives"
    );
    for (input, output) in modifiers.iter().zip(values) {
        let Value::Struct(input) = input else {
            return Err("original modifier body".into());
        };
        let raw = input
            .field("additiveModifier")
            .ok_or("original additive value")?;
        let Value::Int32(expected) = raw else {
            return Err("original additive value is an Int32".into());
        };
        let value = output
            .pointer("/value/additiveModifier")
            .ok_or("exported additive value")?;
        assert_eq!(
            value["value"], *expected,
            "original modifier Int32 and slot survive"
        );
        trace.check(value, raw, "payload")?;
    }
    Ok(values.len())
}

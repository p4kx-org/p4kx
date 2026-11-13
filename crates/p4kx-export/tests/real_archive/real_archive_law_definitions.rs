// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, ExportDiagnosticSeverity, Kind, build_catalog_from,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::authored_definition_witness::{field, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_canonical_texts::TextIndex;
use crate::real_archive_verification::print_line;

#[path = "real_archive_law_fields.rs"]
mod law;

const LAW_SYSTEM: &str = "libs/foundry/records/lawsystem/*";
const FAMILIES: [(Kind, &str); 3] = [
    (Kind::Infractions, "InfractionDefinition"),
    (Kind::InfractionSets, "InfractionSet"),
    (Kind::Jurisdictions, "Jurisdiction"),
];

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_law_definitions_match_canonical_owned_rules_and_links() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let filter = Filter::new(LAW_SYSTEM);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build scoped law definition catalog")?;
    for diagnostic in built.summary().export_health().diagnostics() {
        if diagnostic.severity != ExportDiagnosticSeverity::Information {
            assert!(
                diagnostic
                    .kind
                    .is_some_and(|kind| !FAMILIES.iter().any(|(law, _)| *law == kind)),
                "law extraction has a blocking {:?} diagnostic",
                diagnostic.code
            );
            print_line(&format!(
                "scoped diagnostic catalog also reports {:?} {:?}: {}",
                diagnostic.kind, diagnostic.code, diagnostic.count
            ));
        }
    }
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let origins = Origins::read(&catalog, entry.origin())?;
    let texts = TextIndex::new(&catalog)?;
    let mut total = 0;
    for (kind, source_type) in FAMILIES {
        let counts = verify_family(&forge, &catalog, kind, source_type, &origins, &texts)?;
        print_line(&format!(
            "{kind}: {} physical definitions, {} typed field and array-element checks",
            counts.records, counts.checks
        ));
        total += counts.records;
    }
    assert!(total > 0, "a positive original law source selection");
    watch
        .check()
        .map_err(|_| "Archive changed during law verification")?;
    Ok(())
}

#[derive(Default)]
struct Counts {
    records: usize,
    checks: usize,
}

fn verify_family(
    forge: &DataForge<'_>,
    catalog: &Value,
    kind: Kind,
    source_type: &str,
    origins: &Origins<'_>,
    texts: &TextIndex<'_>,
) -> TestResult<Counts> {
    let mut expected = BTreeSet::new();
    let filter = Filter::new(LAW_SYSTEM);
    for record in forge.records() {
        if record.type_name()? == source_type {
            let path = record
                .record_path()
                .map_err(|_| "original law Record path")?;
            assert!(filter.matches(path), "all law Record paths are in scope");
            expected.insert(record.index());
        }
    }
    let mut order = Vec::new();
    let mut builder = TreeBuilder::new();
    let rows = consumer::rows(catalog, kind)?;
    let mut counts = Counts::default();
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, kind, "recordIndex")?
                .as_u64()
                .ok_or("physical definition index")?,
        )?;
        assert!(
            expected.remove(&index),
            "one row per original physical definition"
        );
        let record = forge.record(index).ok_or("original law Record")?;
        let guid = consumer::identity(row, kind)?;
        assert!(guid == record.guid().to_string(), "exact original law GUID");
        let path = consumer::present(field(row, "recordPath")?)?;
        assert!(
            path.as_str() == Some(record.record_path()?),
            "exact original law path"
        );
        order.push((guid, index));
        let tree = builder
            .build(&record)
            .map_err(|_| "read original law Record body")?;
        let witness = law::Witness::new(forge, &tree, origins, texts);
        witness.verify(kind, tree.root(), row)?;
        counts.records += 1;
        counts.checks += witness.checks();
    }
    assert!(expected.is_empty(), "every physical definition of {kind}");
    assert!(
        order.is_sorted(),
        "GUID and physical ordinal order of {kind}"
    );
    Ok(counts)
}

pub(super) fn verify_class<'source>(
    source: Option<&Original<'source>>,
    row: &Value,
    verify: impl FnOnce(&StructValue<'source>, &Value) -> TestResult,
) -> TestResult {
    let Some(Original::Struct(source)) = source else {
        return verify_state(source, row);
    };
    let payload = consumer::source_value(row)?.ok_or("expected original class payload")?;
    let source_type = consumer::source_value(field(payload, "sourceType")?)?;
    assert_eq!(
        source_type.and_then(Value::as_str),
        Some(source.type_name()),
        "original declared rule type"
    );
    verify(source, payload)
}

pub(super) fn verify_scalar(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let expected = match source {
        Some(Original::Int32(value)) => json!(value),
        Some(Original::Bool(value)) => json!(value),
        Some(Original::Enum(value) | Original::String(value)) => json!(value),
        Some(Original::Single(value)) => return verify_single(*value, row),
        _ => return verify_state(source, row),
    };
    assert!(
        consumer::source_value(row)? == Some(&expected),
        "actual source scalar"
    );
    Ok(())
}

fn verify_single(value: f32, row: &Value) -> TestResult {
    let payload = consumer::source_value(row)?.ok_or("original Single payload")?;
    let bits = value.to_bits();
    assert_eq!(
        field(payload, "rawBits")?,
        &json!(format!("{bits:08x}")),
        "exact original Single bits"
    );
    let finite = Option::<f32>::deserialize(field(payload, "finite")?)?;
    assert_eq!(
        finite.map(f32::to_bits),
        value.is_finite().then_some(bits),
        "the finite view round-trips to the original bits"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, StructValue, TreeBuilder,
    Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

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
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build law definition catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    for (kind, source_type) in FAMILIES {
        let count = verify_family(&forge, &catalog, kind, source_type)?;
        print_line(&format!("{kind}: {count} physical definitions checked"));
    }
    watch
        .check()
        .map_err(|_| "Archive changed during law verification")?;
    Ok(())
}

fn verify_family(
    forge: &DataForge<'_>,
    catalog: &Value,
    kind: Kind,
    source_type: &str,
) -> TestResult<usize> {
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == source_type))
        .map(|record| record.index())
        .collect();
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut builder = TreeBuilder::new();
    let rows = consumer::rows(catalog, kind)?;
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, kind, "recordIndex")?
                .as_u64()
                .ok_or("physical definition index")?,
        )?;
        assert!(seen.insert(index), "one row per physical definition");
        let record = forge.record(index).ok_or("original law Record")?;
        assert_eq!(
            record.type_name().ok(),
            Some(source_type),
            "exact original definition type"
        );
        let guid = consumer::identity(row, kind)?;
        assert_eq!(guid, record.guid().to_string(), "exact original GUID");
        order.push((guid, index));
        let tree = builder
            .build(&record)
            .map_err(|_| "read original law Record body")?;
        verify_body(forge, kind, &tree, row)?;
    }
    assert_eq!(seen, expected, "every physical definition of {kind}");
    assert!(
        order.is_sorted(),
        "GUID and physical ordinal order of {kind}"
    );
    Ok(rows.len())
}

fn verify_body(
    forge: &DataForge<'_>,
    kind: Kind,
    tree: &RecordTree<'_>,
    row: &Value,
) -> TestResult {
    match kind {
        Kind::Infractions => {
            verify_array(
                tree.field("triggers"),
                field(row, "triggers")?,
                verify_scalar,
            )?;
            verify_class(
                tree.field("defaultParameters"),
                field(row, "defaultParameters")?,
                verify_parameters,
            )
        }
        Kind::InfractionSets => verify_array(
            tree.field("infractions"),
            field(row, "infractions")?,
            |slot, row| verify_use(forge, slot, row),
        ),
        Kind::Jurisdictions => {
            for name in [
                "baseFine",
                "earlyPaymentPeriod",
                "isPrison",
                "respectsParentJurisdictionLaws",
            ] {
                verify_scalar(tree.field(name), field(row, name)?)?;
            }
            verify_scalar(
                tree.field("maxStolenGoodsPossessionSCU"),
                field(row, "maxStolenGoodsPossessionScu")?,
            )?;
            for (name, target) in [
                ("parentJurisdiction", "Jurisdiction"),
                ("journalEntry", "JournalEntry"),
            ] {
                verify_link(forge, target, tree.field(name), field(row, name)?)?;
            }
            for (name, target) in [
                ("infractionSets", "InfractionSet"),
                ("prohibitedGoods", "EntityClassDefinition"),
                ("prohibitedResources", "ResourceType"),
            ] {
                verify_array(tree.field(name), field(row, name)?, |slot, row| {
                    verify_link(forge, target, slot, row)
                })?;
            }
            verify_array(
                tree.field("infractions"),
                field(row, "infractions")?,
                |slot, row| verify_use(forge, slot, row),
            )
        }
        _ => Err("kind has no law definition witness".into()),
    }
}

fn verify_use(forge: &DataForge<'_>, source: Option<&Original<'_>>, row: &Value) -> TestResult {
    verify_class(source, row, |original, row| {
        verify_link(
            forge,
            "InfractionDefinition",
            original.field("definition"),
            field(row, "definition")?,
        )?;
        verify_class(
            original.field("parameterOverrides"),
            field(row, "parameterOverrides")?,
            verify_parameters,
        )
    })
}

fn verify_parameters(source: &StructValue<'_>, row: &Value) -> TestResult {
    for name in [
        "isFelony",
        "graceAllowance",
        "gracePeriod",
        "displayGraceTime",
        "earlyPaymentPeriod",
        "pressChargesNotificationTime",
        "removeTimeSeconds",
        "ignoreIfAgainstPartyMember",
        "hideCrimeInJournal",
    ] {
        verify_scalar(source.field(name), field(row, name)?)?;
    }
    Ok(())
}

fn verify_array(
    source: Option<&Original<'_>>,
    row: &Value,
    verify: impl Fn(Option<&Original<'_>>, &Value) -> TestResult,
) -> TestResult {
    let Some(Original::Array(slots)) = source else {
        return verify_state(source, row);
    };
    let payload = consumer::source_value(row)?
        .and_then(Value::as_array)
        .ok_or("expected original array payload")?;
    assert_eq!(
        payload.len(),
        slots.len(),
        "every original array occurrence"
    );
    for (slot, row) in slots.iter().zip(payload) {
        verify(Some(slot), row)?;
    }
    Ok(())
}

fn verify_class(
    source: Option<&Original<'_>>,
    row: &Value,
    verify: impl FnOnce(&StructValue<'_>, &Value) -> TestResult,
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

fn verify_link(
    forge: &DataForge<'_>,
    expected_type: &str,
    source: Option<&Original<'_>>,
    row: &Value,
) -> TestResult {
    let (guid, status) = match source {
        Some(Original::Reference(target)) => {
            let record = forge
                .record(target.record_index())
                .ok_or("original link target")?;
            let status = if matches_ancestry(record.struct_type()?, expected_type)? {
                "present"
            } else {
                "unexpectedTargetType"
            };
            (target.guid(), status)
        }
        Some(Original::Error(ValueError::DanglingReference { guid })) => (*guid, "dangling"),
        _ => return verify_state(source, row),
    };
    let payload = consumer::source_value(row)?.ok_or("expected original Reference payload")?;
    let actual = consumer::source_value(field(payload, "guid")?)?;
    assert_eq!(
        actual,
        Some(&json!(guid.to_string())),
        "exact Reference GUID"
    );
    assert_eq!(
        field(payload, "status")?,
        &json!(status),
        "actual original link state"
    );
    Ok(())
}

fn matches_ancestry(mut actual: p4kx_extraction::Struct<'_>, expected: &str) -> TestResult<bool> {
    let mut seen = BTreeSet::new();
    while seen.insert(actual.index()) {
        if actual.name()? == expected {
            return Ok(true);
        }
        let Some(parent) = actual.parent() else {
            return Ok(false);
        };
        actual = parent;
    }
    Err("cycle in original target type ancestry".into())
}

fn verify_scalar(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let expected = match source {
        Some(Original::Int32(value)) => json!(value),
        Some(Original::Bool(value)) => json!(value),
        Some(Original::Enum(value)) => json!(value),
        Some(Original::Single(value)) => return verify_single(*value, row),
        _ => return verify_state(source, row),
    };
    assert_eq!(
        consumer::source_value(row)?,
        Some(&expected),
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

fn verify_state(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let expected = match source {
        None => "absent",
        Some(Original::Null) => "null",
        Some(Original::Error(_)) => "unreadable",
        Some(_) => "invalid",
    };
    assert!(
        consumer::source_value(row)?.is_none(),
        "missing original payload"
    );
    assert_eq!(field(row, "state")?, &json!(expected), "exact source state");
    Ok(())
}

fn field<'row>(row: &'row Value, name: &str) -> TestResult<&'row Value> {
    row.get(name)
        .ok_or_else(|| format!("missing catalog field {name}").into())
}

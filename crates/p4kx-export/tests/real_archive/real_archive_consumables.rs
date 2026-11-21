// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{BuiltCatalog, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    SourceEncryption, SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn consumable_subtypes_match_the_original_effect_slots_and_values() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let filter = Filter::new(&std::env::var("P4KX_CONSUMABLE_SUBTYPE_FILTER")?);
    let output = scoped_catalog(&archive, &entry, &filter)?;

    let catalog: Value = serde_json::from_slice(output.json())?;
    let (subtypes, verified) = compare_subtypes(&entry, &forge, &catalog, &filter, &watch)?;
    watch.check()?;
    print_line(&format!(
        "consumable source parity: {subtypes} subtypes; {verified:?} authored effect slots"
    ));
    Ok(())
}

fn compare_subtypes<'source>(
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    catalog: &Value,
    filter: &Filter,
    watch: &ArchiveWatch,
) -> TestResult<(usize, BTreeMap<&'source str, usize>)> {
    let rows = catalog
        .pointer("/definitions/consumableSubtypes/rows")
        .and_then(Value::as_array)
        .ok_or("canonical consumable subtype rows")?;
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("the original DataForge comes from the Archive".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let expected = expected_indices(forge, filter)?;
    assert!(!expected.is_empty(), "positive original subtype scope");
    let mut verified = BTreeMap::new();
    for row in rows {
        watch.poll()?;
        let index = u32::try_from(
            row.pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                .ok_or("physical subtype Record index")?,
        )?;
        let tree = forge
            .record(index)
            .ok_or("original subtype owner")?
            .tree()?;
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        assert_eq!(
            tree.type_name(),
            "ConsumableSubtype",
            "exact original subtype Record"
        );
        trace.header(
            row.get("recordIndex").ok_or("sourced Record index")?,
            "recordIndex",
            "selector",
        )?;
        compare(row, tree.field("effectsPerMicroSCU"), &trace, &mut verified)?;
    }
    let exported = rows
        .iter()
        .filter_map(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64));
    assert_eq!(
        expected,
        exported.collect::<Vec<_>>(),
        "every physical subtype in scope is included"
    );
    assert!(!verified.is_empty(), "positive original effect witnesses");
    Ok((rows.len(), verified))
}

fn expected_indices(forge: &DataForge<'_>, filter: &Filter) -> TestResult<Vec<u64>> {
    let mut expected = Vec::new();
    for record in forge.records() {
        if record.type_name().ok() == Some("ConsumableSubtype")
            && filter.matches(record.record_path()?)
        {
            expected.push(u64::from(record.index()));
        }
    }
    Ok(expected)
}

fn compare<'source>(
    row: &Value,
    field: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
    counts: &mut BTreeMap<&'source str, usize>,
) -> TestResult {
    let actual = row.get("effectsPerMicroSCU").ok_or("sourced effect list")?;
    let original = match field {
        None => {
            assert_eq!(
                actual.get("state").and_then(Value::as_str),
                Some("absent"),
                "no original effects declaration"
            );
            return Ok(());
        }
        Some(Original::Null) => {
            assert_eq!(
                actual.get("state").and_then(Value::as_str),
                Some("null"),
                "the original effect list is null"
            );
            return Ok(());
        }
        Some(Original::Array(slots)) => slots,
        Some(_) => return Err("unexpected original effects list storage".into()),
    };
    let exported = actual["value"]
        .as_array()
        .ok_or("original effect list output")?;
    assert_eq!(
        original.len(),
        exported.len(),
        "preserve physical effect order and count"
    );
    for (slot, exported) in original.iter().zip(exported) {
        match slot {
            Original::Null => assert_eq!(
                exported.get("state").and_then(Value::as_str),
                Some("null"),
                "a null source slot keeps its place"
            ),
            Original::Struct(effect) => {
                let body = exported.get("value").ok_or("authored effect body")?;
                compare_effect(effect, body, trace)?;
                *counts.entry(effect.type_name()).or_default() += 1;
            }
            _ => return Err("unexpected original effect slot storage".into()),
        }
    }
    Ok(())
}

fn compare_effect<'source>(
    source: &StructValue<'source>,
    row: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let (kind, singles) = match source.type_name() {
        "ConsumableEffectModifyActorStatus" => (
            "modifyActorStatus",
            &["statPointChange", "statCooldownChange"][..],
        ),
        "ConsumableEffectHealth" => ("health", &["healthChange"][..]),
        "ConsumableEffectAddBuffEffect" => ("addBuffEffect", &[][..]),
        "ConsumableEffectResource" => ("resource", &["amount"][..]),
        "ConsumableEffectGas" => ("gas", &[][..]),
        _ => ("authored", &[][..]),
    };
    assert_eq!(row["kind"], kind, "exact concrete effect family");
    assert_eq!(
        proof::text(row, "/sourceType/value")?,
        source.type_name(),
        "actual derived type"
    );
    for &field in singles {
        exact_single(source, row, field, field, trace)?;
    }
    if let Some(Original::Struct(gas)) = source.field("gasMass") {
        exact_single(
            gas,
            row.pointer("/gasMass/value").ok_or("gas mass body")?,
            "Mass",
            "mass",
            trace,
        )?;
    }
    for (outer, amount) in [
        ("buffDurationOverride", "durationOverride"),
        ("buffValueOverride", "valueOverride"),
    ] {
        if let Some(Original::Struct(body)) = source.field(outer) {
            exact_single(
                body,
                row.get(outer)
                    .and_then(|cell| cell.get("value"))
                    .ok_or("buff override body")?,
                amount,
                "amount",
                trace,
            )?;
        }
    }
    Ok(())
}

fn exact_single<'source>(
    source: &StructValue<'source>,
    row: &Value,
    field: &str,
    mapped: &str,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let carrier = row.get(mapped).ok_or("sourced original Single")?;
    let (raw, number) = match source.field(field) {
        Some(raw @ Original::Single(number)) => (raw, number),
        None => {
            assert_eq!(
                proof::text(carrier, "/state")?,
                "absent",
                "a missing original field is absent"
            );
            return Ok(());
        }
        Some(_) => return Err("unexpected source Single storage".into()),
    };
    let bits = proof::text(carrier, "/value/rawBits")?;
    assert_eq!(
        u32::from_str_radix(bits, 16)?,
        number.to_bits(),
        "original float bits"
    );
    trace.check(carrier, raw, "payload")
}

fn scoped_catalog(
    archive: &Archive,
    entry: &DataForgeEntry,
    filter: &Filter,
) -> TestResult<BuiltCatalog> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    Ok(build_catalog_from(archive, entry, &options)?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn consumable_component_retains_original_bounded_owner_fields() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let path = std::env::var("P4KX_CONSUMABLE_OWNER_RECORD")?;
    let filter = Filter::new(&path);
    let output = scoped_catalog(&archive, &entry, &filter)?;
    let catalog: Value = serde_json::from_slice(output.json())?;
    let rows = catalog
        .pointer("/configurations/consumables/rows")
        .and_then(Value::as_array)
        .ok_or("consumable component rows")?;
    let mut verified = 0;
    for row in rows {
        let owner = u32::try_from(
            row.pointer("/ownerRecordIndex/value")
                .and_then(Value::as_u64)
                .ok_or("owner record index")?,
        )?;
        let record = forge
            .record(owner)
            .ok_or("original physical component owner")?;
        if record.record_path()? != path {
            continue;
        }
        let tree = record.tree()?;
        let slot = usize::try_from(
            row.pointer("/componentOrdinal/value")
                .and_then(Value::as_u64)
                .ok_or("component ordinal")?,
        )?;
        let Some(Original::Array(components)) = tree.field("Components") else {
            return Err("original Entity Components array".into());
        };
        let Some(Original::Struct(component)) = components.get(slot) else {
            return Err("original component slot".into());
        };
        verify_component(component, row)?;
        verified += 1;
    }
    assert!(verified > 0, "positive exact consumable component witness");
    watch.check()?;
    print_line(&format!(
        "consumable inputs: {verified} original component owners"
    ));
    Ok(())
}

fn verify_component(component: &StructValue<'_>, row: &Value) -> TestResult {
    assert_eq!(
        component.type_name(),
        "SCItemConsumableParams",
        "physical component type"
    );
    let Some(Original::Struct(volume)) = component.field("consumableVolume") else {
        return Err("authored consumable volume".into());
    };
    let Some(Original::Int32(micro)) = volume.field("microSCU") else {
        return Err("authored micro-SCU storage".into());
    };
    assert_eq!(
        row.pointer("/consumableVolume/value/microSCU/value")
            .and_then(Value::as_i64),
        Some(i64::from(*micro)),
        "no cargo-unit normalization"
    );
    for name in [
        "containerClosed",
        "canBeReclosed",
        "oneShotConsume",
        "discardWhenConsumed",
    ] {
        let Some(Original::Bool(value)) = component.field(name) else {
            return Err("authored consume flag".into());
        };
        assert_eq!(
            row.get(name)
                .and_then(|cell| cell.get("value"))
                .and_then(Value::as_bool),
            Some(*value),
            "authored flag {name}"
        );
    }
    verify_composition(component, row)
}

fn verify_composition(component: &StructValue<'_>, row: &Value) -> TestResult {
    let Some(Original::Array(contents)) = component.field("defaultContents") else {
        return Err("positive original subtype composition".into());
    };
    let actual = row
        .pointer("/defaultContents/value")
        .and_then(Value::as_array)
        .ok_or("canonical composition")?;
    assert_eq!(
        contents.len(),
        actual.len(),
        "composition retains authored positions"
    );
    for (original, rendered) in contents.iter().zip(actual) {
        let Original::Struct(slot) = original else {
            return Err("original composition slot".into());
        };
        let Some(Original::Single(ratio)) = slot.field("ratio") else {
            return Err("original composition ratio".into());
        };
        let actual = proof::text(rendered, "/value/ratio/value/rawBits")?;
        assert_eq!(
            u32::from_str_radix(actual, 16)?,
            ratio.to_bits(),
            "ratio exact bits"
        );
    }
    Ok(())
}

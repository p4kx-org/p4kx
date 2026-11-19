// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, Filter, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

#[path = "mining_tool_sdk_witness.rs"]
mod sdk;

#[path = "mineable_entity_sdk_witness.rs"]
mod mineable_sdk;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and exact mining laser and attachable Record paths"]
fn mining_and_attachable_configurations_match_the_original_dataforge() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let DataForgeOrigin::Archive(original) = entry.origin() else {
        return Err("actual DataForge Archive Entry".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(original), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    for (selector, table, kind) in [
        (
            "P4KX_MINING_LASER_RECORD",
            "miningLasers",
            "SEntityComponentMiningLaserParams",
        ),
        (
            "P4KX_ATTACHABLE_MODIFIER_RECORD",
            "attachableModifiers",
            "EntityComponentAttachableModifierParams",
        ),
    ] {
        let path = std::env::var(selector).map_err(|_| format!("set exact {selector}"))?;
        let filter = Filter::parse(&format!("*{path}"))?;
        let mut records = forge
            .records()
            .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)));
        let record = records
            .next()
            .ok_or("actual selected EntityClassDefinition")?;
        if records.next().is_some() || record.type_name()? != "EntityClassDefinition" {
            return Err("selector must admit one original physical EntityClassDefinition".into());
        }
        let tree = record.tree()?;
        let catalog = catalog(&archive, &entry, &filter)?;
        let uses = proof::indexed(&catalog, "/provenance/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        let (row, original) = component(&catalog, &tree, table, kind)?;
        let count = if table == "miningLasers" {
            check_laser(&catalog, row, original, &trace)?
        } else {
            check_attachable(row, original, &trace)?
        };
        print_line(&format!(
            "mining source {table}: {count} exact source checks"
        ));
    }
    watch.check().map_err(|_| "Archive changed")?;
    Ok(())
}

fn catalog(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> TestResult<Value> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "produce bounded original mining catalog")?;
    Ok(serde_json::from_slice(built.json())?)
}

fn component<'value, 'source>(
    catalog: &'value Value,
    tree: &'value RecordTree<'source>,
    table: &str,
    kind: &str,
) -> TestResult<(&'value Value, &'value StructValue<'source>)> {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("actual original Components array".into());
    };
    let [row] = proof::rows(catalog, &format!("/configurations/{table}/rows"))? else {
        return Err("one physical component row".into());
    };
    assert_eq!(
        proof::number(row, "/ownerRecordIndex/value")?,
        u64::from(tree.record_index()),
        "physical original owner",
    );
    let slot = usize::try_from(proof::number(row, "/componentOrdinal/value")?)?;
    let Some(Original::Struct(original)) = components.get(slot) else {
        return Err("original component slot".into());
    };
    assert_eq!(
        original.type_name(),
        kind,
        "physical original component type"
    );
    Ok((row, original))
}

fn check_laser<'source>(
    catalog: &Value,
    row: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    for key in ["throttleLerpSpeed", "throttleMinimum"] {
        single(row, original, trace, key)?;
    }
    let raw @ Original::Reference(reference) = original
        .field("globalParams")
        .ok_or("original global Reference")?
    else {
        return Err("a resolved original MiningLaserGlobalParams Reference".into());
    };
    let cell = row
        .get("globalParams")
        .ok_or("canonical global Reference")?;
    trace.check(cell, raw, "selector")?;
    assert_eq!(
        proof::number(cell, "/value/targetRecordIndex/value")?,
        u64::from(reference.record_index()),
        "original global Reference physical index",
    );
    let [global] = proof::rows(catalog, "/definitions/miningLaserGlobals/rows")? else {
        return Err("selected laser's one linked global definition".into());
    };
    assert_eq!(
        proof::number(global, "/source/recordIndex/value")?,
        u64::from(reference.record_index()),
        "resolved global remains available despite source path filter",
    );
    let (values, original) = substructure(
        row,
        original,
        "miningLaserModifiers",
        "miningLaserModifiers",
    )?;
    let mut matched = 0;
    for key in [
        "laserInstability",
        "resistanceModifier",
        "optimalChargeWindowSizeModifier",
    ] {
        if let Ok((value, original)) = substructure(values, original, key, key) {
            single(value, original, trace, "value")?;
            matched += 1;
        }
    }
    if matched == 0 {
        return Err("require a sample with an original populated float modifier".into());
    }
    Ok(matched + 3)
}

fn check_attachable<'source>(
    row: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let raw @ Original::Enum(activation) = original
        .field("activationMethod")
        .ok_or("original modifier activation")?
    else {
        return Err("an original activation Enum".into());
    };
    let cell = row
        .get("activationMethod")
        .ok_or("canonical activation label")?;
    assert_eq!(
        proof::text(cell, "/value")?,
        *activation,
        "authored activation label"
    );
    trace.check(cell, raw, "payload")?;
    let Some(Original::Array(original_slots)) = original.field("modifiers") else {
        return Err("original modifiers array".into());
    };
    let slots = proof::rows(row, "/modifiers/value")?;
    assert_eq!(slots.len(), original_slots.len(), "physical slot count");
    let mut typed = 0;
    for (slot, original) in slots.iter().zip(original_slots.iter()) {
        let Original::Struct(body) = original else {
            continue;
        };
        trace.part(slot, original, "$type", "selector")?;
        let expected = match body.type_name() {
            "ItemMiningModifierParams" => "miningLaser",
            "ItemMineableRockModifierParams" => "mineableRock",
            "MiningFilterItemModifierParams" => "miningFilter",
            "ItemWeaponModifiersParams" => "weapon",
            _ => "authored",
        };
        assert_eq!(
            proof::text(slot, "/value/type")?,
            expected,
            "original concrete effect"
        );
        assert_eq!(
            proof::text(slot, "/value/value/sourceType/value")?,
            body.type_name(),
            "original type is retained in each branch",
        );
        typed += usize::from(expected != "authored");
        if expected == "weapon" {
            typed += check_weapon(slot, body, trace)?;
        }
    }
    if typed == 0 {
        return Err("sample must positively exercise a typed original attachable modifier".into());
    }
    Ok(typed + 1)
}

fn check_weapon<'source>(
    slot: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let row = slot
        .pointer("/value/value")
        .ok_or("canonical weapon modifier")?;
    let (row, original) = substructure(row, original, "weaponModifier", "weaponModifier")?;
    let (stats, original) = substructure(row, original, "weaponStats", "weaponStats")?;
    single(stats, original, trace, "damageMultiplier")?;
    let mut compared = 1;
    for (group, keys) in [
        (
            "salvageModifier",
            &[
                "radiusMultiplier",
                "salvageSpeedMultiplier",
                "extractionEfficiency",
            ][..],
        ),
        (
            "tractorModifier",
            &[
                "forceMultiplier",
                "fullStrengthDistMultiplier",
                "maxDistMultiplier",
                "maxVolumeMultiplier",
            ][..],
        ),
    ] {
        if !matches!(original.field(group), Some(Original::Struct(_))) {
            continue;
        }
        let (row, original) = substructure(stats, original, group, group)?;
        for key in keys {
            single(row, original, trace, key)?;
            compared += 1;
        }
    }
    Ok(compared)
}

fn substructure<'value, 'source>(
    row: &'value Value,
    original: &'value StructValue<'source>,
    source: &str,
    key: &str,
) -> TestResult<(&'value Value, &'value StructValue<'source>)> {
    let Some(Original::Struct(body)) = original.field(source) else {
        return Err("original concrete mining structure".into());
    };
    Ok((
        row.get(key)
            .and_then(|value| value.get("value"))
            .ok_or("canonical structure")?,
        body,
    ))
}

fn single<'source>(
    row: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
    key: &str,
) -> TestResult {
    let raw @ Original::Single(number) = original.field(key).ok_or("original field")? else {
        return Err("original Single".into());
    };
    let cell = row.get(key).ok_or("canonical original field")?;
    assert!(
        u32::from_str_radix(proof::text(cell, "/value/rawBits")?, 16)? == number.to_bits(),
        "{key} retained the exact original bits",
    );
    trace.check(cell, raw, "payload")
}

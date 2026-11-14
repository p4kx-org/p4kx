// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    Reference, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;
const EMP: &str = "SCItemEMPParams";
const NUMBERS: &[&str] = &[
    "chargeTime",
    "unleashTime",
    "cooldownTime",
    "empRadius",
    "minEmpRadius",
    "physRadius",
    "minPhysRadius",
    "distortionDamage",
    "pressure",
];
const PARTICLES: &[(&str, &str)] = &[
    ("ChargedParticle", "chargedParticle"),
    ("ChargingParticle", "chargingParticle"),
];
const STATES: &[&str] = &[
    "idleState",
    "chargingState",
    "chargedState",
    "releasingState",
];
const REFERENCES: &[(&str, &str)] = &[
    ("ChargingTag", "chargingTag"),
    ("ChargedTag", "chargedTag"),
    ("StartChargingTrigger", "startChargingTrigger"),
    ("StopChargingTrigger", "stopChargingTrigger"),
    ("StartChargedTrigger", "startChargedTrigger"),
    ("StopChargedTrigger", "stopChargedTrigger"),
    ("StartUnleashTrigger", "startUnleashTrigger"),
    ("StopUnleashTrigger", "stopUnleashTrigger"),
];

#[test]
#[ignore = "real Archives: set P4KX_DATA_P4K and P4KX_DATA_P4K_B and pass --ignored"]
fn both_installed_archives_match_original_emp_configuration() -> Check {
    let Some(pair) = real_archive_pair().map_err(|_| "configure installed Archives")? else {
        return Err("configure both installed Archives".into());
    };
    let key = real_archive_key().map_err(|_| "read configured Archive key")?;
    for archive_path in <[_; 2]>::from(pair) {
        let archive = Archive::open_with_key(archive_path, Some(key.clone()))
            .map_err(|_| "open configured Archive")?;
        compare_archive(&archive)?;
    }
    Ok(())
}

fn compare_archive(archive: &Archive) -> Check {
    let watch = ArchiveWatch::new(archive);
    let entry = DataForgeEntry::read(archive).map_err(|_| "read configured DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let owner = representative(&forge, &watch)?;
    let path = owner
        .record_path()
        .map_err(|_| "read original Record path")?;
    let filter = Filter::new(path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, &entry, &options)
        .map_err(|_| "build path-scoped EMP catalog")?;
    let item_health = built.summary().export_health();
    let items = item_health
        .kind(Kind::Items)
        .ok_or("selected Item health")?;
    assert!(
        items.source_inventory_complete
            && items.field_inventory_complete
            && items.failed_records().is_empty()
            && items.failed_entries().is_empty(),
        "the original Item inventory and fields are accounted for"
    );
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    compare_owner(&owner, entry.origin(), &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    Ok(())
}

fn representative<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> Check<p4kx_extraction::Record<'source>> {
    let rule = Kind::Items.rule().ok_or("Item selection rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", EMP)
        .ok_or("original EMP declaration")?;
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if components::selected_item(&record, &rule, &filter)?
            && components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            return Ok(record);
        }
    }
    Err("an original selected owner must hold an EMP component".into())
}

fn compare_owner(
    owner: &p4kx_extraction::Record<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
) -> Check {
    let tree = owner.tree().map_err(|_| "read original EMP owner tree")?;
    let Some(Original::Array(originals)) = tree.field("Components") else {
        return Err("original EMP component array".into());
    };
    assert_eq!(
        proof::text(catalog, "/configurations/emp/availability")?,
        "available",
        "the selected source family is available"
    );
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(original_entry) = origin else {
        return Err("observed Archive origin".into());
    };
    let store = SourceUses::new();
    let scope = store
        .observe(SourceInput::Archive(original_entry), InputScope::Payload)
        .map_err(|_| "observe original DataForge")?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(&tree, &uses, &scope, encrypted);
    let mut rows = components::component_rows(catalog, "/configurations/emp/rows")?;
    assert!(!rows.is_empty(), "at least one original EMP row");
    for (slot, raw) in originals.iter().enumerate() {
        let Original::Struct(component) = raw else {
            continue;
        };
        if component.type_name() != EMP {
            continue;
        }
        let row = rows
            .remove(&(tree.record_index(), slot))
            .ok_or("one row per physical EMP slot")?;
        require_equal(
            proof::text(row, "/ownerGuid/value")?,
            &tree.guid().to_string(),
            "original owner GUID",
        )?;
        require_equal(
            proof::text(row, "/sourceType/value")?,
            EMP,
            "original component type",
        )?;
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
        trace.part(cell(row, "sourceType")?, raw, "$type", "selector")?;
        compare_fields(row, component, &trace)?;
    }
    assert!(rows.is_empty(), "no unmatched exported EMP occurrences");
    Ok(())
}

fn compare_fields<'source>(
    row: &Value,
    source: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> Check {
    for name in NUMBERS {
        let output = cell(row, name)?;
        components::single(source, name, output)?;
        if let Some(raw) = source.field(name) {
            trace.check(output, raw, "payload")?;
        }
    }
    for &(name, key) in PARTICLES {
        compare_particle(cell(row, key)?, source.field(name), trace)?;
    }
    for &name in STATES {
        compare_link(cell(row, name)?, source.field(name), trace, true)?;
    }
    for &(name, key) in REFERENCES {
        compare_link(cell(row, key)?, source.field(name), trace, false)?;
    }
    Ok(())
}

fn compare_particle<'source>(
    cell: &Value,
    raw: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
) -> Check {
    match raw {
        None => require_state(cell, "absent"),
        Some(raw @ Original::Null) => {
            require_state(cell, "null")?;
            trace.check(cell, raw, "selector")
        }
        Some(raw @ Original::Struct(particle)) => {
            require_state(cell, "present")?;
            trace.part(cell, raw, "$type", "selector")?;
            trace.part(cell, raw, "$target", "selector")?;
            require_equal(
                proof::text(cell, "/value/sourceType/value")?,
                particle.type_name(),
                "original particle Class",
            )?;
            let Some(path @ Original::String(text)) = particle.field("path") else {
                return Err("expected original particle path field".into());
            };
            let payload = cell
                .pointer("/value/path")
                .ok_or("exported particle path")?;
            require_state(payload, "present")?;
            assert!(
                proof::text(payload, "/value")? == *text,
                "exact authored particle path"
            );
            trace.check(payload, path, "payload")
        }
        _ => Err("unreadable original particle declaration".into()),
    }
}

pub(super) fn compare_link<'source>(
    cell: &Value,
    raw: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
    weak: bool,
) -> Check {
    let Some(raw) = raw else {
        return require_state(cell, "absent");
    };
    match raw {
        Original::Null => {
            require_state(cell, "null")?;
            trace.check(cell, raw, "selector")?;
        }
        Original::Reference(reference) => {
            require_state(cell, "present")?;
            trace.check(cell, raw, "selector")?;
            compare_target(cell, reference, weak)?;
        }
        Original::Error(ValueError::DanglingReference { guid }) if !weak => {
            require_state(cell, "present")?;
            trace.part(cell, raw, "$error.guid", "selector")?;
            require_equal(
                proof::text(cell, "/value/status")?,
                "dangling",
                "dangling Reference status",
            )?;
            require_equal(
                proof::text(cell, "/value/guid/value")?,
                &guid.to_string(),
                "original dangling Reference GUID",
            )?;
        }
        _ => return Err("unreadable original EMP link".into()),
    }
    trace.part(cell, raw, "$dataType", "selector")?;
    trace.part(cell, raw, "$target", "selector")
}

fn compare_target(cell: &Value, target: &Reference<'_>, weak: bool) -> Check {
    if proof::number(cell, "/value/targetRecordIndex/value")? != u64::from(target.record_index()) {
        return Err("original link target Record index".into());
    }
    require_equal(
        proof::text(cell, "/value/recordPath/value")?,
        target.record_path(),
        "original link target Record path",
    )?;
    if weak {
        require_equal(
            proof::text(cell, "/value/recordGuid/value")?,
            &target.guid().to_string(),
            "original weak target Record GUID",
        )?;
        require_equal(
            proof::text(cell, "/value/target/sourceType/value")?,
            target.type_name(),
            "original weak target type",
        )?;
        if let Some(location) = target.location() {
            require_equal(
                proof::text(cell, "/value/target/type")?,
                "instance",
                "original owned-instance target",
            )?;
            require_equal(
                proof::text(cell, "/value/target/location/value")?,
                location.as_str(),
                "original owned-instance target location",
            )?;
        } else {
            require_equal(
                proof::text(cell, "/value/target/type")?,
                "record",
                "original Record target",
            )?;
        }
    } else {
        require_equal(
            proof::text(cell, "/value/status")?,
            "present",
            "original Reference resolution",
        )?;
        require_equal(
            proof::text(cell, "/value/guid/value")?,
            &target.guid().to_string(),
            "original Reference GUID",
        )?;
        require_equal(
            proof::text(cell, "/value/targetType/value")?,
            target.type_name(),
            "original Reference target type",
        )?;
    }
    Ok(())
}

fn require_equal(actual: &str, expected: &str, mismatch: &'static str) -> Check {
    if actual == expected {
        Ok(())
    } else {
        Err(mismatch.into())
    }
}

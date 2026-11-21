// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope, Record,
    RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value;

use crate::original_item_arena as arena;
use crate::real_archive_item_components::{self as components, cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::{real_archive_shop_inventory_v3, real_archive_verification};

#[path = "real_archive_quantum_interdiction_graph.rs"]
mod graph;

pub(super) type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;
const DEVICE: &str = "SCItemQuantumInterdictionGeneratorParams";
const TABLE: &str = "/configurations/quantumInterdiction";
const ROWS: &str = "/configurations/quantumInterdiction/rows";
const JAMMER: &str = "SCItemQuantumJammerParams";
const PULSE: &str = "SCItemQuantumInterdictionPulseParams";
const ROOT_NUMBERS: &[&str] = &[
    "basePowerDrawFraction",
    "pulsePowerFraction",
    "jammerPowerFraction",
];
const JAMMER_NUMBERS: &[&str] = &["jammerRange", "maxPowerDraw", "greenZoneCheckRange"];
const PULSE_NUMBERS: &[(&str, &str)] = &[
    ("chargeTimeSecs", "chargeTimeSeconds"),
    ("dischargeTimeSecs", "dischargeTimeSeconds"),
    ("cooldownTimeSecs", "cooldownTimeSeconds"),
    ("radiusMeters", "radiusMeters"),
    (
        "decreaseChargeRateTimeSeconds",
        "decreaseChargeRateTimeSeconds",
    ),
    (
        "increaseChargeRateTimeSeconds",
        "increaseChargeRateTimeSeconds",
    ),
    (
        "activationPhaseDuration_seconds",
        "activationPhaseDurationSeconds",
    ),
    ("disperseChargeTimeSeconds", "disperseChargeTimeSeconds"),
    ("maxPowerDraw", "maxPowerDraw"),
    (
        "stopChargingPowerDrawFraction",
        "stopChargingPowerDrawFraction",
    ),
    (
        "maxChargeRatePowerDrawFraction",
        "maxChargeRatePowerDrawFraction",
    ),
    ("activePowerDrawFraction", "activePowerDrawFraction"),
    ("tetheringPowerDrawFraction", "tetheringPowerDrawFraction"),
    ("greenZoneCheckRange", "greenZoneCheckRange"),
];
const PULSE_LINKS: &[(&str, &str)] = &[
    ("startChargingIP", "startChargingInteraction"),
    ("cancelChargingIP", "cancelChargingInteraction"),
    ("disperseChargeIP", "disperseChargeInteraction"),
];

#[test]
#[ignore = "real Archives: set P4KX_DATA_P4K and P4KX_DATA_P4K_B and pass --ignored"]
fn both_installed_archives_match_original_quantum_interdiction_configuration() -> Check {
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
    let entry = DataForgeEntry::read(archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let owner = representative(&forge, &watch)?;
    let filter = Filter::new(owner.record_path().map_err(|_| "original QIG owner path")?);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built =
        build_catalog_from(archive, &entry, &options).map_err(|_| "build scoped QIG catalog")?;
    let items = built
        .summary()
        .export_health()
        .kind(Kind::Items)
        .ok_or("selected Item health")?;
    assert!(
        items.source_inventory_complete
            && items.field_inventory_complete
            && items.failed_records().is_empty()
            && items.failed_entries().is_empty(),
        "the QIG owner selection has a complete original field inventory"
    );
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "decode scoped QIG catalog")?;
    real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    let (devices, coverage) = compare_owner(&owner, entry.origin(), &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during QIG verification")?;
    real_archive_verification::print_line(&format!(
        "QIG source comparison: {devices} device slots, {} state slots, {} transition slots",
        coverage.states, coverage.transitions,
    ));
    Ok(())
}

fn representative<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> Check<Record<'source>> {
    let rule = Kind::Items.rule().ok_or("Item selection rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", DEVICE)
        .ok_or("original QIG declaration")?;
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if components::selected_item(&record, &rule, &filter)?
            && components::has_component(&mut trees, &record, std::slice::from_ref(&probe))?
        {
            let tree = trees
                .build(&record)
                .map_err(|_| "read original QIG owner")?;
            if devices(&tree).any(|(_, _, device)| suitable(device)) {
                return Ok(record);
            }
        }
    }
    Err("expected a QIG owner with both device classes and timed/value graph transitions".into())
}

fn suitable(device: &StructValue<'_>) -> bool {
    [("jammerSettings", JAMMER), ("quantumInterdictionPulseSettings", PULSE)]
        .into_iter()
        .all(|(field, nominal)| matches!(device.field(field), Some(Original::Struct(value)) if value.type_name() == nominal))
        && graph::representative(device.field("visualGraphParams"))
}

fn devices<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> impl Iterator<Item = (usize, &'tree Original<'source>, &'tree StructValue<'source>)> {
    let values = match tree.field("Components") {
        Some(Original::Array(values)) => values.as_ref(),
        _ => &[],
    };
    values
        .iter()
        .enumerate()
        .filter_map(|(slot, raw)| match raw {
            Original::Struct(device) if device.type_name() == DEVICE => Some((slot, raw, device)),
            _ => None,
        })
}

fn compare_owner(
    owner: &Record<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
) -> Check<(usize, graph::Coverage)> {
    let tree = owner.tree().map_err(|_| "read QIG owner Record tree")?;
    let mut rows = components::component_rows(catalog, ROWS)?;
    assert!(
        proof::text(catalog, &format!("{TABLE}/availability"))? == "available",
        "independently selected QIG table is available"
    );
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(input) = origin else {
        return Err("original DataForge Archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(input), InputScope::Payload)?;
    let marker = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(&tree, &uses, &scope, marker);
    let mut coverage = graph::Coverage::default();
    let mut count = 0;
    for (slot, raw, device) in devices(&tree) {
        let row = rows
            .remove(&(tree.record_index(), slot))
            .ok_or("corresponding physical QIG slot")?;
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
        arena::source_type(&trace, raw, device, row)?;
        compare_device(device, row, &trace, &mut coverage)?;
        count += 1;
    }
    assert!(
        count > 0 && rows.is_empty() && coverage.complete(),
        "one exact selected owner retains every QIG row and both original graph selector variants"
    );
    Ok((count, coverage))
}

fn compare_device<'source>(
    device: &StructValue<'source>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    coverage: &mut graph::Coverage,
) -> Check {
    for name in ROOT_NUMBERS {
        single(device, row, name, name, trace)?;
    }
    for name in ["mainDeviceSwitchOn", "mainDeviceSwitchOff"] {
        link(device.field(name), cell(row, name)?, trace, Link::Weak)?;
    }
    owned(
        device.field("jammerSettings"),
        cell(row, "jammerSettings")?,
        trace,
        JAMMER,
        |source, row| {
            for name in JAMMER_NUMBERS {
                single(source, row, name, name, trace)?;
            }
            for name in ["setJammerSwitchOn", "setJammerSwitchOff"] {
                link(source.field(name), cell(row, name)?, trace, Link::Weak)?;
            }
            Ok(())
        },
    )?;
    owned(
        device.field("quantumInterdictionPulseSettings"),
        cell(row, "pulseSettings")?,
        trace,
        PULSE,
        |source, row| {
            for &(name, wire) in PULSE_NUMBERS {
                single(source, row, name, wire, trace)?;
            }
            for &(name, wire) in PULSE_LINKS {
                link(source.field(name), cell(row, wire)?, trace, Link::Weak)?;
            }
            Ok(())
        },
    )?;
    graph::compare(
        device.field("visualGraphParams"),
        cell(row, "visualGraph")?,
        trace,
        coverage,
    )
}

pub(super) fn single<'source>(
    source: &StructValue<'source>,
    row: &Value,
    name: &str,
    wire: &str,
    trace: &Trace<'_, 'source>,
) -> Check {
    let output = cell(row, wire)?;
    components::single(source, name, output)?;
    if let Some(raw) = source.field(name) {
        trace.check(output, raw, "payload")?;
    }
    Ok(())
}

pub(super) fn owned<'source>(
    original: Option<&Original<'source>>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    nominal: &str,
    compare: impl FnOnce(&StructValue<'source>, &Value) -> Check,
) -> Check {
    match original {
        None => require_state(row, "absent"),
        Some(raw @ Original::Null) => {
            require_state(row, "null")?;
            trace.check(row, raw, "selector")?;
            trace.part(row, raw, "$target", "selector")
        }
        Some(raw @ Original::Struct(value)) => {
            assert!(
                value.type_name() == nominal,
                "declared inline or strong target matches"
            );
            require_state(row, "present")?;
            trace.part(row, raw, "$type", "selector")?;
            trace.part(row, raw, "$target", "selector")?;
            let payload = cell(row, "value")?;
            arena::source_type(trace, raw, value, payload)?;
            compare(value, payload)
        }
        _ => Err("read original owned QIG structure".into()),
    }
}

#[derive(Clone, Copy)]
pub(super) enum Link {
    Record,
    Weak,
}

pub(super) fn link<'source>(
    original: Option<&Original<'source>>,
    row: &Value,
    trace: &Trace<'_, 'source>,
    kind: Link,
) -> Check {
    super::real_archive_emp::compare_link(row, original, trace, matches!(kind, Link::Weak))
}

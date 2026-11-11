// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, KindRule, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, DataType, Filter,
    InputScope, PointerProbe, Record, RecordTree, SourceEncryption, SourceInput, SourceUses,
    StructValue, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type Rows<'catalog> = BTreeMap<(u32, usize), &'catalog Value>;

const FILTER: &str = "libs/foundry/records/entities/*";
const KINDS: &[(&str, &str)] = &[
    ("SSensorMineComponentParams", "sensorMine"),
    ("SExplosiveOrdnanceParams", "ordnance"),
    ("SCItemBombParams", "bomb"),
    ("SCItemSpaceMineParams", "spaceMine"),
    ("SCItemMissileParams", "missile"),
];
const ORDNANCE: &[(&str, &str)] = &[
    ("requiresLauncher", "requiresLauncher"),
    ("enableLifetime", "enableLifetime"),
    ("maxLifetime", "maxLifetime"),
    ("armTime", "armTime"),
    ("maxArmableOverride", "maxArmableOverride"),
    ("igniteTime", "igniteTime"),
    ("collisionDelayTime", "collisionDelayTime"),
    ("explosionSafetyDistance", "explosionSafetyDistance"),
    ("projectileProximity", "projectileProximity"),
];
const LASER: &[(&str, &str)] = &[
    ("LaserLength", "laserLength"),
    ("LaserHitOffset", "laserHitOffset"),
];
const PROXIMITY: &[(&str, &str)] = &[
    ("MaxRaysPerMine", "maxRaysPerMine"),
    ("Radius", "radius"),
    ("WarningRadius", "warningRadius"),
    ("AngleVertical", "angleVertical"),
    ("AngleHorizontal", "angleHorizontal"),
    ("GuideLaserLength", "guideLaserLength"),
];

#[derive(Default)]
struct Counts {
    devices: usize,
    triggers: usize,
    scalars: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn explosive_components_match_actual_item_slots_and_sensor_source_fields() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let filter = Filter::new(FILTER);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build selected Item catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let counts = compare(&forge, entry.origin(), &catalog, &filter, &watch)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    if std::env::var_os("P4KX_EXPECT_EXPLOSIVE_COMPONENTS").is_some() && counts.devices == 0 {
        return Err("configured Build must exercise an original explosive component".into());
    }
    real_archive_verification::print_line(&format!(
        "explosive components: {} device slots, {} trigger slots, {} original scalar fields",
        counts.devices, counts.triggers, counts.scalars,
    ));
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    filter: &Filter,
    watch: &ArchiveWatch,
) -> TestResult<Counts> {
    let mut rows = exported_rows(catalog)?;
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("expected an observed Archive DataForge".into());
    };
    let sources = SourceUses::new();
    let scope = sources
        .observe(SourceInput::Archive(entry), InputScope::Payload)
        .map_err(|_| "observe actual DataForge origin")?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let probes: Vec<_> = KINDS
        .iter()
        .filter_map(|(name, _)| forge.pointer_probe("Components", name))
        .collect();
    let mut trees = TreeBuilder::new();
    let mut counts = Counts::default();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        if !selected_item(&record, &rule, filter)? || !has_device(&mut trees, &record, &probes)? {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original explosive owner")?;
        let trace = Trace::of(&tree, &uses, scope.source_ref().as_str(), encrypted);
        compare_components(&tree, &trace, &mut rows, &mut counts)?;
    }
    assert!(
        rows.is_empty(),
        "every exported device has its independently selected original slot"
    );
    if counts.devices > 0 {
        assert!(
            proof::text(catalog, "/configurations/explosives/availability")? == "available",
            "original device instances prove the source family is available"
        );
    }
    Ok(counts)
}

fn selected_item(record: &Record<'_>, rule: &KindRule, filter: &Filter) -> TestResult<bool> {
    let path = record
        .record_path()
        .map_err(|_| "read original Record path")?;
    if !filter.matches(path) || !rule.matches_path(path) {
        return Ok(false);
    }
    let source_type = record
        .type_name()
        .map_err(|_| "read original Record type")?;
    let name = record.name().map_err(|_| "read original Record name")?;
    Ok(source_type == rule.type_name && !is_kind_leftover(Kind::Items, path, name))
}

fn has_device<'source>(
    trees: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    probes: &[PointerProbe<'source>],
) -> TestResult<bool> {
    for probe in probes {
        if trees
            .holds(probe, record)
            .map_err(|_| "inspect original Components array")?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn exported_rows(catalog: &Value) -> TestResult<Rows<'_>> {
    let mut rows = Rows::new();
    let mut previous = None;
    for row in proof::rows(catalog, "/configurations/explosives/rows")? {
        let owner = u32::try_from(proof::number(row, "/ownerRecordIndex/value")?)?;
        let slot = usize::try_from(proof::number(row, "/componentOrdinal/value")?)?;
        let position = (owner, slot);
        if previous.is_some_and(|key| key >= position) || rows.insert(position, row).is_some() {
            return Err("exported device slots must be unique and follow physical order".into());
        }
        previous = Some(position);
    }
    Ok(rows)
}

fn compare_components<'source>(
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    rows: &mut Rows<'_>,
    counts: &mut Counts,
) -> TestResult {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("the observed device owner has no readable Components array".into());
    };
    for (slot, value) in components.iter().enumerate() {
        let Original::Struct(component) = value else {
            continue;
        };
        let Some(&(_, kind)) = KINDS
            .iter()
            .find(|(name, _)| *name == component.type_name())
        else {
            continue;
        };
        let row = rows
            .remove(&(tree.record_index(), slot))
            .ok_or("one exported row for each original device slot")?;
        compare_identity(tree, trace, value, component, kind, row)?;
        let config = row
            .pointer("/configuration/value/value")
            .ok_or("typed device configuration")?;
        counts.devices += 1;
        if kind == "sensorMine" {
            counts.scalars += compare_fields(
                trace,
                component,
                config,
                &[("FriendlyFire", "friendlyFire")],
            )?;
            counts.scalars += compare_trigger(trace, component, cell(config, "triggerType")?)?;
            counts.triggers += 1;
        } else {
            counts.scalars += compare_fields(trace, component, config, ORDNANCE)?;
        }
    }
    Ok(())
}

fn compare_identity<'source>(
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    original: &Original<'source>,
    component: &StructValue<'source>,
    kind: &str,
    row: &Value,
) -> TestResult {
    assert!(
        proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
        "the exported owner is the original physical Record"
    );
    assert!(
        proof::text(row, "/sourceType/value")? == component.type_name()
            && proof::text(row, "/configuration/value/type")? == kind,
        "the device shape follows the original concrete component type"
    );
    trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
    trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
    trace.check(cell(row, "componentOrdinal")?, original, "conversion")?;
    trace.part(cell(row, "sourceType")?, original, "$type", "selector")?;
    trace.part(cell(row, "configuration")?, original, "$type", "selector")
}

fn compare_trigger<'source>(
    trace: &Trace<'_, 'source>,
    component: &StructValue<'source>,
    exported: &Value,
) -> TestResult<usize> {
    let Some(field) = component
        .fields()
        .iter()
        .find(|field| field.name() == "TriggerType")
    else {
        require_state(exported, "absent")?;
        return Ok(0);
    };
    let value = field.value();
    trace.part(exported, value, "$dataType", "selector")?;
    if matches!(value, Original::Error(_)) {
        require_state(exported, "unreadable")?;
        trace.part(exported, value, "$value", "attempted")?;
        return Ok(0);
    }
    if field.storage().data_type() != DataType::StrongPointer {
        require_state(exported, "invalid")?;
        trace.check(exported, value, "selector")?;
        return Ok(0);
    }
    match value {
        Original::Null => {
            require_state(exported, "null")?;
            trace.check(exported, value, "selector")?;
            Ok(0)
        }
        Original::Struct(trigger) => compare_trigger_shape(trace, value, trigger, exported),
        _ => Err("unexpected actual TriggerType representation".into()),
    }
}

fn compare_trigger_shape<'source>(
    trace: &Trace<'_, 'source>,
    value: &Original<'source>,
    trigger: &StructValue<'source>,
    exported: &Value,
) -> TestResult<usize> {
    trace.part(exported, value, "$type", "selector")?;
    let (kind, fields) = match trigger.type_name() {
        "SSensorMineTriggerType" => ("base", &[][..]),
        "SSensorMineLaserTrigger" => ("laser", LASER),
        "SSensorMineProximityTrigger" => ("proximity", PROXIMITY),
        _ => {
            require_state(exported, "unsupported")?;
            return Ok(0);
        }
    };
    require_state(exported, "present")?;
    assert!(
        proof::text(exported, "/value/type")? == kind,
        "the trigger discriminator follows its actual original type"
    );
    if fields.is_empty() {
        return Ok(0);
    }
    let fields_row = exported
        .pointer("/value/value")
        .ok_or("typed trigger fields")?;
    compare_fields(trace, trigger, fields_row, fields)
}

fn compare_fields<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    exported: &Value,
    mappings: &[(&str, &str)],
) -> TestResult<usize> {
    let mut count = 0;
    for &(name, key) in mappings {
        let exported = cell(exported, key)?;
        let Some(value) = source.field(name) else {
            require_state(exported, "absent")?;
            continue;
        };
        compare_scalar(trace, value, exported)?;
        count += 1;
    }
    Ok(count)
}

fn compare_scalar<'source>(
    trace: &Trace<'_, 'source>,
    value: &Original<'source>,
    exported: &Value,
) -> TestResult {
    let retained = exported.get("value");
    let (state, role, equal) = match value {
        Original::Bool(value) => (
            "present",
            "payload",
            retained.and_then(Value::as_bool) == Some(*value),
        ),
        Original::Int32(value) => (
            "present",
            "payload",
            retained.and_then(Value::as_i64) == Some(i64::from(*value)),
        ),
        Original::Single(value) => {
            let bits = proof::text(exported, "/value/rawBits")?;
            (
                "present",
                "payload",
                u32::from_str_radix(bits, 16).ok() == Some(value.to_bits()),
            )
        }
        Original::Null => ("null", "selector", true),
        Original::Error(_) => {
            require_state(exported, "unreadable")?;
            return trace.part(exported, value, "$value", "attempted");
        }
        _ => ("invalid", "selector", true),
    };
    require_state(exported, state)?;
    assert!(
        equal,
        "the exported scalar preserves the exact original representation"
    );
    trace.check(exported, value, role)
}

fn require_state(cell: &Value, expected: &str) -> TestResult {
    assert!(
        proof::text(cell, "/state")? == expected,
        "the carrier keeps the original read state"
    );
    Ok(())
}

fn cell<'row>(row: &'row Value, name: &str) -> TestResult<&'row Value> {
    row.get(name)
        .ok_or_else(|| "expected the authored field carrier".into())
}

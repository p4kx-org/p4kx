// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

#[path = "worn_gear_sdk_witness.rs"]
mod sdk;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SUIT: &str = "SCItemSuitArmorParams";
const CLOTHING: &str = "SCItemClothingParams";
const RADAR: &str = "SCItemRadarComponentParams";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and exact gear Record paths"]
fn worn_gear_matches_original_components_and_damage_resistance_macro() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("original DataForge Archive Entry".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    for (name, table, expected, count) in [
        ("P4KX_SUIT_ARMOR_RECORD", "suitArmor", SUIT, 26),
        ("P4KX_CLOTHING_RECORD", "clothing", CLOTHING, 5),
        ("P4KX_ITEM_RADAR_RECORD", "itemRadars", RADAR, 4),
    ] {
        let path = std::env::var(name).map_err(|_| format!("set exact Record path in {name}"))?;
        let tree = exact_entity(&forge, &path)?;
        let filter = Filter::parse(&format!("*{path}"))?;
        let options = CatalogOptions {
            filter: Some(&filter),
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let built = build_catalog_from(&archive, &entry, &options)
            .map_err(|_| "produce catalog for selected original gear Record")?;
        let catalog = serde_json::from_slice(built.json())?;
        let uses = proof::indexed(&catalog, "/provenance/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        let (row, raw, component) = match_component(&catalog, table, expected, &tree)?;
        trace.check(
            row.get("componentOrdinal")
                .ok_or("physical component slot")?,
            raw,
            "conversion",
        )?;
        let verified = compare_values(row, component, &trace, expected)?;
        let verified = if expected == SUIT {
            verified
                + compare_macro(&catalog, &forge, row, component, &trace, |tree| {
                    Trace::of(tree, &uses, &scope, encrypted)
                })?
        } else {
            verified
        };
        assert_eq!(verified, count, "positive exact value count for {table}");
        print_line(&format!(
            "worn gear {table}: {verified} original typed values checked"
        ));
    }
    watch.check().map_err(|_| "Archive changed")?;
    Ok(())
}

fn exact_entity<'source>(
    forge: &'source DataForge<'source>,
    path: &str,
) -> TestResult<RecordTree<'source>> {
    let mut candidates = forge
        .records()
        .filter(|record| record.record_path() == Ok(path));
    let candidate = candidates.next().ok_or("original named gear Record")?;
    if candidates.next().is_some() || candidate.type_name()? != "EntityClassDefinition" {
        return Err("exact gear path must identify one physical Entity Record".into());
    }
    let filter = Filter::parse(&format!("*{path}"))?;
    if forge
        .records()
        .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
        .take(2)
        .count()
        != 1
    {
        return Err("gear Record filter must admit one exact Record".into());
    }
    candidate
        .tree()
        .map_err(|_| "read selected original gear Record".into())
}

fn match_component<'row, 'source>(
    catalog: &'row Value,
    table: &str,
    expected: &str,
    tree: &'row RecordTree<'source>,
) -> TestResult<(
    &'row Value,
    &'row Original<'source>,
    &'row StructValue<'source>,
)> {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original Components array".into());
    };
    let rows = proof::rows(catalog, &format!("/configurations/{table}/rows"))?;
    let [row] = rows else {
        return Err("one canonical gear component row".into());
    };
    assert_eq!(
        proof::number(row, "/ownerRecordIndex/value")?,
        u64::from(tree.record_index()),
        "physical original owning Record"
    );
    let slot = usize::try_from(proof::number(row, "/componentOrdinal/value")?)?;
    let raw = components
        .get(slot)
        .ok_or("original physical component slot")?;
    let component = body(raw)?;
    assert_eq!(component.type_name(), expected, "original component type");
    Ok((row, raw, component))
}

type FieldPairs = &'static [(&'static str, &'static str)];

struct GearSection {
    source: &'static str,
    key: &'static str,
    fields: FieldPairs,
}

const SUIT_SCALARS: FieldPairs = &[("integrityMilestoneToBreak", "integrityMilestoneToBreak")];
const NO_SCALARS: FieldPairs = &[];
const CLOTHING_SECTIONS: &[GearSection] = &[
    GearSection {
        source: "TemperatureResistance",
        key: "temperatureResistance",
        fields: &[
            ("MinResistance", "minResistance"),
            ("MaxResistance", "maxResistance"),
        ],
    },
    GearSection {
        source: "RadiationResistance",
        key: "radiationResistance",
        fields: &[
            ("MaximumRadiationCapacity", "maximumRadiationCapacity"),
            ("RadiationDissipationRate", "radiationDissipationRate"),
        ],
    },
    GearSection {
        source: "Flight",
        key: "flight",
        fields: &[("gForceResistance", "gForceResistance")],
    },
];
const RADAR_SECTIONS: &[GearSection] = &[
    GearSection {
        source: "aimAssist",
        key: "aimAssist",
        fields: &[
            ("distanceMinAssignment", "distanceMinAssignment"),
            ("distanceMaxAssignment", "distanceMaxAssignment"),
            ("outsideRangeBufferDistance", "outsideRangeBufferDistance"),
        ],
    },
    GearSection {
        source: "pingProperties",
        key: "pingProperties",
        fields: &[("cooldownTime", "cooldownTime")],
    },
];
const MACRO_SCALARS: FieldPairs = &[("IgnoreMeleeDamage", "ignoreMeleeDamage")];
const MACRO_CHANNELS: FieldPairs = &[
    ("PhysicalResistance", "physical"),
    ("EnergyResistance", "energy"),
    ("DistortionResistance", "distortion"),
    ("ThermalResistance", "thermal"),
    ("BiochemicalResistance", "biochemical"),
    ("StunResistance", "stun"),
];
const MACRO_CHANNEL_FIELDS: FieldPairs = &[
    ("Multiplier", "multiplier"),
    ("Threshold", "threshold"),
    ("DamageCap", "damageCap"),
    ("DamageCapLimit", "damageCapLimit"),
];

fn specification(expected: &str) -> Option<(FieldPairs, &'static [GearSection])> {
    match expected {
        SUIT => Some((SUIT_SCALARS, &[])),
        CLOTHING => Some((NO_SCALARS, CLOTHING_SECTIONS)),
        RADAR => Some((NO_SCALARS, RADAR_SECTIONS)),
        _ => None,
    }
}

fn all_gear_values_present(original: &StructValue<'_>, kind: &str) -> bool {
    specification(kind).is_some_and(|(scalars, groups)| {
        field_types_present(original, scalars)
            && groups
                .iter()
                .all(|group| section_present(original, group.source, group.fields))
    })
}

fn macro_values_present(tree: &RecordTree<'_>) -> bool {
    let Some(Original::Struct(original)) = tree.field("damageResistance") else {
        return false;
    };
    field_types_present(original, MACRO_SCALARS)
        && MACRO_CHANNELS
            .iter()
            .all(|&(source, _)| section_present(original, source, MACRO_CHANNEL_FIELDS))
}

fn section_present(original: &StructValue<'_>, section: &str, names: FieldPairs) -> bool {
    matches!(original.field(section), Some(Original::Struct(group)) if field_types_present(group, names))
}

fn field_types_present(original: &StructValue<'_>, names: FieldPairs) -> bool {
    names.iter().all(|&(name, _)| {
        matches!(
            original.field(name),
            Some(Original::Single(_) | Original::Bool(_))
        )
    })
}

fn compare_values<'source>(
    row: &Value,
    component: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
    expected: &str,
) -> TestResult<usize> {
    let (scalars, groups) = specification(expected).ok_or("supported gear component")?;
    let mut count = fields(row, component, trace, scalars)?;
    for group in groups {
        count += nested(row, component, trace, group.source, group.key, group.fields)?;
    }
    Ok(count)
}

fn compare_macro<'trace, 'source>(
    catalog: &Value,
    forge: &'source DataForge<'source>,
    suit: &Value,
    component: &StructValue<'source>,
    owner_trace: &Trace<'_, 'source>,
    trace: impl FnOnce(&RecordTree<'source>) -> Trace<'trace, 'source>,
) -> TestResult<usize> {
    let target = macro_link(suit, component, owner_trace)?;
    let tree = forge
        .record(target)
        .ok_or("physical macro definition")?
        .tree()
        .map_err(|_| "read original resistance macro")?;
    let rows = proof::rows(catalog, "/definitions/damageResistanceMacros/rows")?;
    let [row] = rows else {
        return Err("one admitted linked macro row".into());
    };
    assert_eq!(
        proof::number(row, "/source/recordIndex/value")?,
        u64::from(tree.record_index()),
        "filtered definition came from the physical Reference target",
    );
    count_macro_values(row, &tree, &trace(&tree))
}

fn macro_link<'source>(
    suit: &Value,
    component: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<u32> {
    let raw @ Original::Reference(reference) = component
        .field("damageResistance")
        .ok_or("original suit Reference")?
    else {
        return Err("resolved original DamageResistanceMacro Reference".into());
    };
    let cell = suit
        .get("damageResistance")
        .ok_or("canonical suit Reference")?;
    trace.check(cell, raw, "selector")?;
    assert!(
        proof::text(cell, "/value/status")? == "present"
            && proof::text(cell, "/value/guid/value")? == reference.guid().to_string()
            && proof::number(cell, "/value/targetRecordIndex/value")?
                == u64::from(reference.record_index()),
        "the authored suit Reference resolves to the original physical macro and GUID"
    );
    Ok(reference.record_index())
}

fn count_macro_values<'source>(
    row: &Value,
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let original = tree
        .field("damageResistance")
        .ok_or("original macro body")?;
    let cell = row.get("damageResistance").ok_or("canonical macro body")?;
    trace.part(cell, original, "$type", "selector")?;
    let contents = cell.get("value").ok_or("decoded macro body")?;
    let original = body(original)?;
    let mut count = fields(contents, original, trace, MACRO_SCALARS)?;
    for &(source, key) in MACRO_CHANNELS {
        count += nested(contents, original, trace, source, key, MACRO_CHANNEL_FIELDS)?;
    }
    Ok(count)
}

fn nested<'source>(
    row: &Value,
    owner: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
    name: &str,
    key: &str,
    names: &[(&str, &str)],
) -> TestResult<usize> {
    let raw = owner.field(name).ok_or("original gear structure")?;
    let cell = row.get(key).ok_or("canonical gear structure")?;
    trace.part(cell, raw, "$type", "selector")?;
    fields(
        cell.get("value").ok_or("decoded gear structure")?,
        body(raw)?,
        trace,
        names,
    )
}

fn body<'value, 'source>(
    raw: &'value Original<'source>,
) -> TestResult<&'value StructValue<'source>> {
    let Original::Struct(structure) = raw else {
        return Err("expected original concrete structure".into());
    };
    Ok(structure)
}

fn fields<'source>(
    row: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
    names: &[(&str, &str)],
) -> TestResult<usize> {
    for &(source, key) in names {
        let raw = original.field(source).ok_or("original gear scalar")?;
        let cell = row.get(key).ok_or("canonical gear scalar")?;
        match raw {
            Original::Single(number) => assert!(
                u32::from_str_radix(proof::text(cell, "/value/rawBits")?, 16)? == number.to_bits(),
                "{source} retains exact original Single bits",
            ),
            Original::Bool(flag) => assert!(
                cell.get("value").and_then(Value::as_bool) == Some(*flag),
                "{source} retains original boolean",
            ),
            _ => return Err("expected original gear Single or Bool".into()),
        }
        trace.check(cell, raw, "payload")?;
    }
    Ok(names.len())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_export::catalog_queries::equipment::vehicle_armor::{VehicleArmor, VehicleArmorSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ProvenanceMode,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{ARMOR, DAMAGE, FILTER, TestResult, compare_armor, slots};
use crate::authored_component_witness::ComponentEvidence;
use crate::real_archive_item_components as components;
use crate::real_archive_item_durability::Trace;
use crate::real_archive_verification::print_line;

struct TuningGroup {
    path: &'static [&'static str],
    fields: &'static [(&'static str, &'static str)],
}

const TUNING: [TuningGroup; 4] = [
    TuningGroup {
        path: &["damageMultiplier"],
        fields: &DAMAGE,
    },
    TuningGroup {
        path: &["armorPenetrationResistance"],
        fields: &[("basePenetrationReduction", "basePenetrationReduction")],
    },
    TuningGroup {
        path: &["armorPenetrationResistance", "penetrationAbsorptionForType"],
        fields: &DAMAGE,
    },
    TuningGroup {
        path: &["armorDeflection", "deflectionValue"],
        fields: &DAMAGE,
    },
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and run this test with --ignored"]
fn installed_vehicle_armor_sdk_matches_the_original_physical_tuning() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())
        .map_err(|_| "open the installed Archive")?;
    let build = archive
        .build()
        .ok_or("the installed Archive needs an exact Build")?;
    let identity = [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ];
    assert!(
        identity.iter().all(|part| !part.trim().is_empty()),
        "the vehicle armor witness requires all four Build identity fields"
    );
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify Archive before opening DataForge")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the installed DataForge")?;
    let original = entry.open().map_err(|_| "open the installed DataForge")?;
    let owner = discover(&original, &watch)?;
    let evidence = ComponentEvidence::new(&original, entry.origin(), build)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "prepare the vehicle armor SDK session from the decoded DataForge")?
    .with_provenance_mode(ProvenanceMode::Full);
    let sdk = session
        .parsed(None)
        .map_err(|_| "open the parsed vehicle armor SDK session")?;
    let (occurrences, fields) = sdk
        .with_vehicle_armor(
            CatalogRecordSelection::RecordIndex(owner),
            None,
            |snapshot| compare_snapshot(&evidence, owner, &snapshot),
        )
        .map_err(|_| "read the selected vehicle armor SDK rows")??;
    watch
        .check()
        .map_err(|_| "verify Archive after the vehicle armor witness")?;
    print_line(&format!(
        "Vehicle armor SDK source witness: {}; {occurrences} physical occurrences and {fields} nested values",
        identity.join(" ")
    ));
    Ok(())
}

fn discover<'source>(forge: &'source DataForge<'source>, watch: &ArchiveWatch) -> TestResult<u32> {
    let rule = Kind::Items.rule().ok_or("selected item rule")?;
    let filter = Filter::new(FILTER);
    let probe = forge
        .strict_pointer_probe("Components", ARMOR)
        .ok_or("the installed DataForge must declare vehicle armor")?;
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during armor discovery")?;
        if components::selected_item(&record, &rule, &filter).ok() != Some(true)
            || trees.holds(&probe, &record).ok() != Some(true)
        {
            continue;
        }
        let Ok(tree) = trees.build(&record) else {
            continue;
        };
        if slots(&tree)?.iter().any(|slot| {
            matches!(
                slot,
                Original::Struct(source) if source.type_name() == ARMOR && has_tuning(source)
            )
        }) {
            return Ok(record.index());
        }
    }
    Err("the installed Build needs a readable vehicle armor occurrence with nested tuning".into())
}

fn has_tuning(source: &StructValue<'_>) -> bool {
    TUNING.iter().any(|group| {
        nested(source, group.path).is_some_and(|original| {
            group
                .fields
                .iter()
                .any(|(name, _)| matches!(original.field(name), Some(Original::Single(_))))
        })
    })
}

fn nested<'value, 'source>(
    mut source: &'value StructValue<'source>,
    path: &[&str],
) -> Option<&'value StructValue<'source>> {
    for name in path {
        let Some(Original::Struct(child)) = source.field(name) else {
            return None;
        };
        source = child;
    }
    Some(source)
}

fn compare_snapshot(
    evidence: &ComponentEvidence<'_>,
    owner: u32,
    snapshot: &VehicleArmorSnapshot<'_>,
) -> TestResult<(usize, usize)> {
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "the selected armor has complete scoped field coverage"
    );
    let mut nested_values = 0;
    evidence.verify(
        snapshot,
        owner,
        &[ARMOR],
        VehicleArmor::owner,
        |row, source, trace| {
            assert!(
                row.source_type().value() == Some(&source.type_name()),
                "the typed row agrees with its concrete original component"
            );
            let output = serde_json::to_value(row)?;
            compare_armor(source, &output)?;
            nested_values += trace_tuning(source, &output, trace)?;
            Ok(())
        },
    )?;
    assert!(
        nested_values > 0,
        "the installed witness retained nested authored values"
    );
    Ok((snapshot.rows().len(), nested_values))
}

fn trace_tuning<'source>(
    component: &StructValue<'source>,
    row: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let mut count = 0;
    for group in &TUNING {
        let Some(source) = nested(component, group.path) else {
            continue;
        };
        let mut output = row;
        for name in group.path {
            output = components::cell(output, name)?
                .get("value")
                .ok_or("original nested armor carrier")?;
        }
        for (name, key) in group.fields {
            if let Some(raw @ Original::Single(_)) = source.field(name) {
                let cell = components::cell(output, key)?;
                trace.check(cell, raw, "payload")?;
                count += 1;
            }
        }
    }
    Ok(count)
}

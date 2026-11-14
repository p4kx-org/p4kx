// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use super::{Family, TestResult, report_missing, representative, selected_catalog, verify_family};
use crate::authored_definition_witness::{field, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_verification::print_line;

const SLOT: &str = "LocationResourceSlot";
const COUNT: &str = "numberOfResourceSlotsPerEntity";
const TAG: &str = "resourceSlotTag";
const TARGET: &str = "requiredResourceSlot";
const FAMILY: Family = Family {
    kind: Kind::LocationResourceSlots,
    source_type: SLOT,
    field: None,
};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn resource_slot_definition_and_module_reference_match_original() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let module = module_sample(&forge, &watch)?;
    let slot = if let Some(module) = module {
        let target = verify_module(&archive, &entry, &forge, &module)?;
        let record = forge.record(target).ok_or("original module slot target")?;
        print_line("One original mission module Reference matched its physical slot position.");
        Some(
            record
                .tree()
                .map_err(|_| "read original module slot target")?,
        )
    } else {
        print_line(
            "No resolved mission module slot Reference was observed among exact source-type owners.",
        );
        representative(&forge, &watch, &FAMILY)?
    };
    let Some(slot) = slot else {
        return report_missing(SLOT);
    };
    verify_slot(&archive, &entry, &forge, &slot)?;
    watch
        .check()
        .map_err(|_| "Archive changed during resource slot verification".into())
}

fn module_sample<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<Option<RecordTree<'source>>> {
    let mut builder = TreeBuilder::new();
    for record in forge.records() {
        if record.type_name().ok() != Some("ModuleDeclaration") {
            continue;
        }
        watch.poll().map_err(|_| "Archive changed")?;
        let module = builder
            .build(&record)
            .map_err(|_| "read original module candidate")?;
        let Some(Original::Struct(data)) = module.field("moduleType") else {
            continue;
        };
        if data.type_name() != "ModuleDeclarationType_Mission"
            || module.record_path().contains(['*', '?'])
        {
            continue;
        }
        let Some(Original::Reference(target)) = data.field(TARGET) else {
            continue;
        };
        let Some(slot) = forge.record(target.record_index()) else {
            continue;
        };
        if slot.type_name().ok() == Some(SLOT)
            && !slot
                .record_path()
                .map_err(|_| "original slot path")?
                .contains(['*', '?'])
        {
            return Ok(Some(module));
        }
    }
    Ok(None)
}

fn verify_module(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
) -> TestResult<u32> {
    let catalog = selected_catalog(archive, entry, tree.record_path())?;
    let owner = tree.record_index();
    let row = physical_row(&catalog, Kind::MissionModuleDeclarations, owner)?;
    let Some(Original::Struct(data)) = tree.field("moduleType") else {
        return Err("sampled original module type".into());
    };
    let Some(original @ Original::Reference(target)) = data.field(TARGET) else {
        return Err("sampled original resource slot Reference".into());
    };
    let export = consumer::present(field(row, "moduleType")?)?;
    let cell = field(export, TARGET)?;
    verify_link(forge, SLOT, Some(original), cell)?;
    let output = consumer::present(cell)?;
    assert_eq!(
        consumer::present(field(output, "targetRecordIndex")?)?,
        &json!(target.record_index()),
        "authored module Reference's physical Record index"
    );
    let locations = Locations::read(tree.root());
    let origins = Origins::read(&catalog, entry.origin())?;
    origins.occurrence(cell, owner, locations.ordinal(original)?, "$value")?;
    Ok(target.record_index())
}

fn verify_slot(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
) -> TestResult {
    let catalog = selected_catalog(archive, entry, tree.record_path())?;
    let origins = Origins::read(&catalog, entry.origin())?;
    verify_family(forge, &catalog, &FAMILY, tree, &origins)?;
    let owner = tree.record_index();
    let row = physical_row(&catalog, FAMILY.kind, owner)?;
    let locations = Locations::read(tree.root());
    let count = tree.field(COUNT);
    let count_cell = field(row, COUNT)?;
    if let Some(Original::Int32(value)) = count {
        assert_eq!(
            consumer::present(count_cell)?,
            &json!(value),
            "exact authored count"
        );
    } else {
        verify_state(count, count_cell)?;
    }
    verify_link(forge, "Tag", tree.field(TAG), field(row, TAG)?)?;
    for name in [COUNT, TAG] {
        if let Some(source) = tree.field(name) {
            origins.occurrence(
                field(row, name)?,
                owner,
                locations.ordinal(source)?,
                "$value",
            )?;
        }
    }
    print_line(
        "One original location resource slot matched its physical identity, slot-field states and origins.",
    );
    Ok(())
}

fn physical_row(catalog: &Value, kind: Kind, index: u32) -> TestResult<&Value> {
    consumer::rows(catalog, kind)?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
        })
        .ok_or_else(|| "source-selected physical row".into())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope, Record,
    RecordTree, SourceEncryption, SourceInput, SourceScope, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::original_item_arena as arena;
use crate::original_item_numbers as numbers;
use crate::real_archive_item_components::{self as components, cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::{real_archive_shop_inventory_v3, real_archive_verification};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const DEVICE: &str = "SCItemJumpDriveParams";
const ROWS: &str = "/configurations/jumpDrives/rows";
const NUMBERS: &[&str] = &[
    "alignmentDecayRate",
    "alignmentRate",
    "fuelUsageEfficiencyMultiplier",
    "tuningDecayRate",
    "tuningRate",
];
const DEFINITIONS: &[(&str, &str)] = &[
    ("flightTuning", "/configurations/jumpDriveFlightTuning/rows"),
    ("tunnelForces", "/configurations/jumpTunnelForces/rows"),
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn jump_drive_retains_exact_device_values_and_original_linked_definitions() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let path = witness_path(&forge, &watch)?;
    let filter = Filter::new(path);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build original selected Jump Drive catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let uses = proof::indexed(&catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("expected original Archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let marker = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let witness = Witness {
        catalog: &catalog,
        uses: &uses,
        scope: &scope,
        marker,
    };
    let (count, links) = witness.compare(&forge, &filter)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    real_archive_verification::print_line(&format!(
        "jump drive original source comparison: {count} device slots, {links} definition links"
    ));
    Ok(())
}

fn witness_path<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<&'source str> {
    let rule = Kind::Items.rule().ok_or("item rule")?;
    let filter = Filter::new(components::FILTER);
    let probe = forge
        .pointer_probe("Components", DEVICE)
        .ok_or("Jump Drive declaration")?;
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
                .map_err(|_| "read original device owner")?;
            if stored_devices(&tree).any(|(_, _, device)| {
                DEFINITIONS
                    .iter()
                    .all(|(key, _)| matches!(device.field(key), Some(Original::Reference(_))))
            }) {
                return record
                    .record_path()
                    .map_err(|_| "read original device owner path".into());
            }
        }
    }
    Err("expected an original Jump Drive with both definition links".into())
}

fn stored_devices<'tree, 'source>(
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
            Original::Struct(value) if value.type_name() == DEVICE => Some((slot, raw, value)),
            _ => None,
        })
}

struct Witness<'a> {
    catalog: &'a Value,
    uses: &'a HashMap<&'a str, &'a Value>,
    scope: &'a SourceScope,
    marker: Option<bool>,
}

impl Witness<'_> {
    fn compare(&self, forge: &DataForge<'_>, filter: &Filter) -> TestResult<(usize, usize)> {
        let mut rows = components::component_rows(self.catalog, ROWS)?;
        let rule = Kind::Items.rule().ok_or("item rule")?;
        let mut count = 0;
        let mut links = 0;
        for record in forge.records() {
            if components::selected_item(&record, &rule, filter)? {
                let tree = record.tree().map_err(|_| "read original device")?;
                let trace = Trace::of(&tree, self.uses, self.scope, self.marker);
                for (slot, raw, device) in stored_devices(&tree) {
                    let row = rows
                        .remove(&(record.index(), slot))
                        .ok_or("corresponding original device row")?;
                    trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
                    trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
                    trace.check(cell(row, "componentOrdinal")?, raw, "conversion")?;
                    arena::source_type(&trace, raw, device, row)?;
                    compare_numbers(&trace, device, row)?;
                    for &(key, path) in DEFINITIONS {
                        links += self.compare_definition(forge, &trace, device, row, key, path)?;
                    }
                    count += 1;
                }
            }
        }
        assert!(
            count > 0 && links >= DEFINITIONS.len() && rows.is_empty(),
            "the selected scope covers both target families and every physical component"
        );
        Ok((count, links))
    }

    fn compare_definition<'source>(
        &self,
        forge: &DataForge<'source>,
        trace: &Trace<'_, 'source>,
        device: &StructValue<'source>,
        row: &Value,
        key: &str,
        path: &str,
    ) -> TestResult<usize> {
        let link = cell(row, key)?;
        let Some(raw) = device.field(key) else {
            require_state(link, "absent")?;
            return Ok(0);
        };
        let Original::Reference(reference) = raw else {
            require_state(link, "null")?;
            return Ok(0);
        };
        require_state(link, "present")?;
        let index = reference.record_index();
        let target = forge.record(index).ok_or("original linked definition")?;
        trace.check(cell(cell(link, "value")?, "guid")?, raw, "payload")?;
        assert!(
            proof::number(link, "/value/targetRecordIndex/value")? == u64::from(index),
            "the authored target is linked by physical index"
        );
        self.compare_definition_body(&target, path)?;
        Ok(1)
    }

    fn compare_definition_body(&self, original: &Record<'_>, path: &str) -> TestResult {
        let index = original.index();
        let mut candidates = proof::rows(self.catalog, path)?.iter().filter(|row| {
            row.pointer("/source/recordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(index))
        });
        let entry = candidates
            .next()
            .ok_or("original linked definition was exported")?;
        assert!(candidates.next().is_none(), "definition is retained once");
        let tree = original
            .tree()
            .map_err(|_| "read linked definition original body")?;
        let trace = Trace::of(&tree, self.uses, self.scope, self.marker);
        trace.header(cell(cell(entry, "source")?, "guid")?, "guid", "payload")?;
        assert!(
            proof::text(entry, "/source/guid/value")? == tree.guid().to_string(),
            "exact definition identity"
        );
        let coverage = arena::compare(&trace, tree.fields().iter(), cell(entry, "fields")?)?;
        assert_eq!(
            coverage.roots,
            tree.fields().len(),
            "all original definition roots"
        );
        Ok(())
    }
}

fn compare_numbers<'source>(
    trace: &Trace<'_, 'source>,
    device: &StructValue<'source>,
    row: &Value,
) -> TestResult {
    for &field in NUMBERS {
        let raw = device.field(field).ok_or("original numeric device field")?;
        let output = cell(row, field)?;
        require_state(output, "present")?;
        let expected = numbers::expected(raw)?.ok_or("original numeric storage")?;
        assert!(
            numbers::equal(&expected, cell(output, "value")?),
            "original number and bits"
        );
        trace.check(output, raw, "payload")?;
    }
    Ok(())
}

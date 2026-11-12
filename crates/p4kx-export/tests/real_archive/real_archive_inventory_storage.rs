// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceScope, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_components::{self as components, cell, owned, single};
use crate::real_archive_item_durability::{self as proof, Trace};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn inventory_occupancy_and_storage_trace_original_item_and_vehicle_owners() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open()?;
    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("original DCB archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let storage = Verification::new(&forge, &catalog, &scope, encrypted)?;
    storage.verify_storage_item()?;
    storage.verify_bounds()?;
    watch.check()?;
    crate::real_archive_verification::print_line(
        "inventory: original wearable, storage item, definition and vehicle samples checked",
    );
    Ok(())
}

struct Verification<'read, 'source> {
    forge: &'read DataForge<'source>,
    catalog: &'read Value,
    uses: std::collections::HashMap<&'read str, &'read Value>,
    scope: &'read SourceScope,
    encrypted: Option<bool>,
}

impl<'read, 'source> Verification<'read, 'source> {
    fn new(
        forge: &'read DataForge<'source>,
        catalog: &'read Value,
        scope: &'read SourceScope,
        encrypted: Option<bool>,
    ) -> TestResult<Self> {
        Ok(Self {
            forge,
            catalog,
            scope,
            encrypted,
            uses: proof::indexed(catalog, "/provenance/uses", "id")?,
        })
    }

    fn verify_storage_item(&self) -> TestResult {
        let owners = components::component_rows(
            self.catalog,
            "/configurations/inventoryConfigurations/rows",
        )?;
        let mut trees = TreeBuilder::new();
        for ((owner, slot), row) in owners {
            if row["kind"] != "itemStorage"
                || row
                    .pointer("/containerParams/value/status")
                    .and_then(Value::as_str)
                    != Some("present")
            {
                continue;
            }
            let record = self
                .forge
                .record(owner)
                .ok_or("original inventory owner Record")?;
            let tree = trees.build(&record)?;
            let (original, component) = component(&tree, slot)?;
            self.check_owner(row, &tree, original, component)?;
            let Some(Original::Reference(target)) = component.field("containerParams") else {
                return Err("original item storage selector".into());
            };
            return self.verify_definition(row, target.record_index(), &target.guid().to_string());
        }
        Err("configured build contains no traced item with a resolved storage definition".into())
    }

    fn verify_definition(&self, selector: &Value, index: u32, guid: &str) -> TestResult {
        if proof::number(selector, "/containerParams/value/targetRecordIndex/value")?
            != u64::from(index)
            || proof::text(selector, "/containerParams/value/guid/value")? != guid
        {
            return Err("original selector physical identity differs".into());
        }
        let definition = proof::rows(self.catalog, "/definitions/inventoryContainers/rows")?
            .iter()
            .find(|row| {
                row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
            })
            .ok_or("exported physical storage definition")?;
        if proof::text(definition, "/guid/value")? != guid {
            return Err("original storage definition GUID differs".into());
        }
        let record = self
            .forge
            .record(index)
            .ok_or("original inventory definition Record")?;
        let original = record.tree()?;
        assert_eq!(
            original.type_name(),
            "InventoryContainer",
            "the target is a physical storage definition"
        );
        owned(
            original.root(),
            definition,
            "interiorDimensions",
            compare_vector,
        )?;
        owned(
            original.root(),
            definition,
            "inventoryType",
            compare_inventory_type,
        )
    }

    fn verify_bounds(&self) -> TestResult {
        let rows = components::component_rows(self.catalog, "/configurations/itemBounds/rows")?;
        let mut wearable = false;
        let mut vehicle = false;
        let mut trees = TreeBuilder::new();
        for ((owner, slot), row) in rows {
            let (eligible, role) = match row["kind"].as_str() {
                Some("itemOccupancy") => (!wearable, "itemOccupancy"),
                Some("vehicleBounds") => (!vehicle, "vehicleBounds"),
                _ => continue,
            };
            if !eligible {
                continue;
            }
            let record = self
                .forge
                .record(owner)
                .ok_or("original item bounds owner")?;
            let tree = trees.build(&record)?;
            let (original, component) = component(&tree, slot)?;
            if role == "itemOccupancy" {
                let Some(Original::Struct(attachment)) = component.field("AttachDef") else {
                    continue;
                };
                if !matches!(attachment.field("Type"), Some(Original::String(kind) | Original::Enum(kind))
                    if kind.starts_with("Char_Clothing_"))
                {
                    continue;
                }
                self.check_owner(row, &tree, original, component)?;
                owned(component, row, "AttachDef", compare_occupancy)?;
                wearable = true;
            } else {
                self.check_owner(row, &tree, original, component)?;
                owned(component, row, "maxBoundingBoxSize", compare_vector)?;
                vehicle = true;
            }
            if wearable && vehicle {
                return Ok(());
            }
        }
        Err("configured build lacks a traced wearable or vehicle bounds sample".into())
    }

    fn check_owner(
        &self,
        row: &Value,
        tree: &RecordTree<'source>,
        original: &Original<'source>,
        component: &StructValue<'source>,
    ) -> TestResult {
        if proof::text(row, "/ownerGuid/value")? != tree.guid().to_string() {
            return Err("the exported owner GUID differs from the physical source".into());
        }
        assert_eq!(
            proof::text(row, "/sourceType/value")?,
            component.type_name(),
            "the source type agrees with the physical component slot"
        );
        let trace = Trace::of(tree, &self.uses, self.scope, self.encrypted);
        trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
        trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
        trace.check(cell(row, "componentOrdinal")?, original, "conversion")?;
        trace.part(cell(row, "sourceType")?, original, "$type", "selector")
    }
}

fn component<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    slot: usize,
) -> TestResult<(&'tree Original<'source>, &'tree StructValue<'source>)> {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original owner Components".into());
    };
    let original = components
        .get(slot)
        .ok_or("original owner component slot")?;
    let Original::Struct(component) = original else {
        return Err("original owner component structure".into());
    };
    Ok((original, component))
}

fn compare_inventory_type(source: &StructValue<'_>, row: &Value) -> TestResult {
    match source.type_name() {
        "InventoryOpenContainerType" => {
            assert_eq!(row["kind"], "open", "the authored open storage class");
            for field in [
                "minPermittedItemSize",
                "maxPermittedItemSize",
                "gridPosOffset",
            ] {
                owned(source, row, field, compare_vector)?;
            }
        }
        "InventoryClosedContainerType" => {
            assert_eq!(row["kind"], "closed", "the authored closed storage class");
            owned(source, row, "capacity", compare_capacity)?;
        }
        "InventoryContainerTypeBase" => {
            assert_eq!(row["kind"], "base", "the authored base storage class");
        }
        _ => assert_eq!(
            row["kind"], "unknownType",
            "the authored unknownType storage class"
        ),
    }
    Ok(())
}

fn compare_occupancy(source: &StructValue<'_>, row: &Value) -> TestResult {
    owned(source, row, "inventoryOccupancyVolume", compare_capacity)?;
    for field in [
        "inventoryOccupancyDimensions",
        "inventoryOccupancyLocalBoundsMin",
        "inventoryOccupancyLocalBoundsMax",
        "inventoryOccupancyDimensionsUIOverride",
        "inventoryOccupancyFixedGridDimensionsUIV4",
    ] {
        owned(source, row, field, compare_vector)?;
    }
    Ok(())
}

fn compare_vector(source: &StructValue<'_>, row: &Value) -> TestResult {
    for field in ["x", "y"] {
        single(source, field, cell(row, field)?)?;
    }
    if source.type_name() == "Vec3" {
        single(source, "z", cell(row, "z")?)?;
    }
    Ok(())
}

fn compare_capacity(source: &StructValue<'_>, row: &Value) -> TestResult {
    let field = match source.type_name() {
        "SStandardCargoUnit" => {
            return single(
                source,
                "standardCargoUnits",
                cell(row, "standardCargoUnits")?,
            );
        }
        "SCentiCargoUnit" => "centiSCU",
        "SMicroCargoUnit" => "microSCU",
        "SBaseCargoUnit" => {
            assert_eq!(row["kind"], "base", "the authored base storage class");
            return Ok(());
        }
        _ => {
            assert_eq!(
                row["kind"], "unknownUnit",
                "the authored unknownUnit storage class"
            );
            return Ok(());
        }
    };
    let Some(Original::Int32(amount)) = source.field(field) else {
        return Err("original integer cargo amount".into());
    };
    if cell(row, field)?["value"].as_i64() != Some(i64::from(*amount)) {
        return Err("the stored integer cargo amount differs".into());
    }
    Ok(())
}

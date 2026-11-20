// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{DataForge, RecordTree, StructValue, Value as Original};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link};
use crate::catalog_v3_consumer::source_value;
use crate::fuel_capacity::compare_capacity;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_catalog_table_parity::TestResult;
use crate::real_archive_law_definitions::{verify_class, verify_scalar};

pub(super) struct Counts {
    pub(super) checked_cells: usize,
    pub(super) tier_slots: usize,
}

pub(super) fn verify<'source>(
    forge: &'source DataForge<'source>,
    record: &RecordTree<'source>,
    row: &Value,
    origins: &Origins<'_>,
) -> TestResult<Counts> {
    let source = record.root();
    let witness = Witness {
        forge,
        origins,
        locations: Locations::read(source),
        index: record.record_index(),
        checked: Cell::new(0),
    };
    witness.array(source, row, "medBedConfigs", |raw, tier| {
        witness.owned(raw, tier, |raw, tier| witness.tier(raw, tier))
    })?;
    for name in ["drugEfficacyConfig", "availableDrugTypes"] {
        witness.drugs(source, row, name)?;
    }
    witness.owned(
        source.field("resourceConsumption"),
        field(row, "resourceConsumption")?,
        |raw, consumption| witness.consumption(raw, consumption),
    )?;
    let tier_slots = match source.field("medBedConfigs") {
        Some(Original::Array(items)) => items
            .iter()
            .filter(|item| matches!(item, Original::Struct(_)))
            .count(),
        _ => 0,
    };
    Ok(Counts {
        checked_cells: witness.checked.get(),
        tier_slots,
    })
}

struct Witness<'scope, 'catalog, 'source> {
    forge: &'source DataForge<'source>,
    origins: &'scope Origins<'catalog>,
    locations: Locations<'source>,
    index: u32,
    checked: Cell<usize>,
}

impl<'source> Witness<'_, '_, 'source> {
    fn tier(&self, original: &StructValue<'source>, row: &Value) -> TestResult {
        self.scalar(original.field("medBedTier"), field(row, "medBedTier")?)?;
        self.scalar(
            original.field("defaultRespawnRange"),
            field(row, "defaultRespawnRange")?,
        )?;
        self.array(original, row, "statTypesToHeal", |value, slot| {
            self.scalar(value, slot)
        })?;
        self.drugs(original, row, "drugEfficacyForConsumableTypes")
    }

    fn drugs(&self, original: &StructValue<'source>, row: &Value, name: &str) -> TestResult {
        self.array(original, row, name, |raw, cell| {
            self.owned(raw, cell, verify_authored_member_names)
        })
    }

    fn consumption(&self, original: &StructValue<'source>, row: &Value) -> TestResult {
        let raw_link = original.field("bedResource");
        let link = field(row, "bedResource")?;
        self.occurrence(raw_link, link, "$value")?;
        verify_link(self.forge, "ResourceType", raw_link, link)?;
        if let Some(Original::Reference(reference)) = raw_link {
            let value = source_value(link)?.ok_or("original resource link")?;
            let index = source_value(field(value, "targetRecordIndex")?)?.and_then(Value::as_u64);
            if index != Some(u64::from(reference.record_index())) {
                return Err("medical consumption ResourceType physical target differs".into());
            }
        }
        for name in [
            "resourceAmountForSurgeryT1",
            "resourceAmountForSurgeryT2",
            "resourceAmountForSurgeryT3",
            "resourceAmountForCloning",
        ] {
            let raw = original.field(name);
            let cell = field(row, name)?;
            compare_capacity(raw, cell)?;
            self.owned(raw, cell, |raw, unit| self.capacity(raw, unit))?;
        }
        Ok(())
    }

    fn capacity(&self, raw: &StructValue<'source>, unit: &Value) -> TestResult {
        let field_name = match raw.type_name() {
            "SStandardCargoUnit" => "standardCargoUnits",
            "SCentiCargoUnit" => "centiSCU",
            "SMicroCargoUnit" => "microSCU",
            _ => return verify_authored_member_names(raw, unit),
        };
        self.occurrence(raw.field(field_name), field(unit, field_name)?, "$value")
    }

    fn scalar(&self, raw: Option<&Original<'source>>, cell: &Value) -> TestResult {
        self.occurrence(raw, cell, "$value")?;
        if let Some(Original::Int64(expected)) = raw {
            if source_value(cell)?.and_then(Value::as_i64) != Some(*expected) {
                return Err("exact original Int64 medical respawn range differs".into());
            }
            return Ok(());
        }
        verify_scalar(raw, cell)
    }

    fn owned(
        &self,
        raw: Option<&Original<'source>>,
        cell: &Value,
        verify: impl FnOnce(&StructValue<'source>, &Value) -> TestResult,
    ) -> TestResult {
        let part = if matches!(raw, Some(Original::Struct(_))) {
            "$type"
        } else {
            "$value"
        };
        self.occurrence(raw, cell, part)?;
        verify_class(raw, cell, verify)
    }

    fn array(
        &self,
        raw: &StructValue<'source>,
        row: &Value,
        name: &str,
        verify: impl Fn(Option<&Original<'source>>, &Value) -> TestResult,
    ) -> TestResult {
        let original = raw.field(name);
        let cell = field(row, name)?;
        self.occurrence(original, cell, "$value")?;
        verify_array(original, cell, verify)
    }

    fn occurrence(&self, raw: Option<&Original<'source>>, cell: &Value, part: &str) -> TestResult {
        if let Some(value) = raw.filter(|value| !matches!(value, Original::Error(_))) {
            self.origins
                .occurrence(cell, self.index, self.locations.ordinal(value)?, part)?;
            self.checked.set(self.checked.get() + 1);
        }
        Ok(())
    }
}

fn verify_authored_member_names(raw: &StructValue<'_>, row: &Value) -> TestResult {
    let roots = row
        .pointer("/additionalFields/roots")
        .and_then(Value::as_array)
        .ok_or("original declared class member arena")?;
    let names = roots
        .iter()
        .map(|field| field.pointer("/name/value").and_then(Value::as_str));
    if !names.eq(raw.fields().iter().map(|field| Some(field.name()))) {
        return Err("authored class members or occurrence order differ".into());
    }
    Ok(())
}

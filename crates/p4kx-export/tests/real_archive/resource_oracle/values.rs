// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{
    ArchiveWatch, DataForge, DataForgeOrigin, RecordTree, StructValue, TreeBuilder, Value as Raw,
    ValueError,
};
use serde::Serialize;
use serde_json::Value;

use super::carriers::{Carriers, Uses};
use super::{Failure, Result, compare};

const MAX_SAMPLE_ROWS: usize = 64;
const MAX_RESOURCE_ROWS: usize = 16_384;
const MAX_RAW_VALUES: usize = 65_536;
const MAX_RAW_DEPTH: usize = 32;
const ROOT_FIELDS: &[&str] = &[
    "validateDefaultCargoBox",
    "defaultCargoContainers",
    "properties",
];

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Coverage {
    sampled_rows: usize,
    checked_values: usize,
    owned_fields: usize,
    packaging_slots: usize,
    volatility_properties: usize,
    quality_properties: usize,
    quality_values: usize,
    null_box_slots: usize,
    unresolved_boxes: BTreeMap<&'static str, usize>,
}

impl Coverage {
    const fn require_capabilities(&self) -> Result<()> {
        if self.packaging_slots == 0
            || self.volatility_properties == 0
            || self.quality_properties == 0
            || self.quality_values == 0
        {
            return Err(Failure::check("bounded-sample-missing-required-capability"));
        }
        Ok(())
    }
}

pub(super) fn check(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    watch: &ArchiveWatch,
) -> Result<Coverage> {
    if catalog
        .pointer("/definitions/resourceTypes/availability")
        .and_then(Value::as_str)
        != Some("available")
    {
        return Err(Failure::check("resource-source-unavailable"));
    }
    let rows = catalog
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("canonical-resource-table"))?;
    if rows.is_empty() || rows.len() > MAX_RESOURCE_ROWS {
        return Err(Failure::check("resource-row-budget"));
    }
    let uses = Uses::new(catalog, origin)?;
    let mut coverage = Coverage::default();
    let mut fields = BTreeSet::new();
    let mut builder = TreeBuilder::new();
    for row in rows.iter().take(MAX_SAMPLE_ROWS) {
        watch
            .poll()
            .map_err(|error| Failure::source("archive-source-changed", error))?;
        let index = uses.record_index(row)?;
        let record = forge
            .record(index)
            .ok_or_else(|| Failure::check("physical-resource-record"))?;
        let tree = builder
            .build(&record)
            .map_err(|error| Failure::source("raw-resource-tree", error))?;
        identity(&tree, row)?;
        sample(&tree, row, &uses, &mut coverage, &mut fields)?;
        coverage.sampled_rows += 1;
    }
    coverage.owned_fields = fields.len();
    coverage.require_capabilities()?;
    Ok(coverage)
}

fn sample<'catalog>(
    tree: &RecordTree<'_>,
    row: &'catalog Value,
    uses: &Uses<'catalog>,
    coverage: &mut Coverage,
    fields: &mut BTreeSet<(String, String)>,
) -> Result<()> {
    let carriers = Carriers::new(row, uses, tree.record_index())?;
    let mut walker = Walker {
        carriers: &carriers,
        coverage,
        fields,
    };
    for name in ROOT_FIELDS {
        walker
            .fields
            .insert((tree.type_name().to_owned(), (*name).to_owned()));
        walker.walk(tree.field(name), name, 0)?;
    }
    Ok(())
}
fn identity(tree: &RecordTree<'_>, row: &Value) -> Result<()> {
    if tree.type_name() != "ResourceType"
        || row.pointer("/guid/value").and_then(Value::as_str)
            != Some(tree.guid().to_string().as_str())
        || row.pointer("/recordPath/value").and_then(Value::as_str) != Some(tree.record_path())
    {
        return Err(Failure::check("physical-resource-identity"));
    }
    Ok(())
}

struct Walker<'walk, 'catalog> {
    carriers: &'walk Carriers<'catalog>,
    coverage: &'walk mut Coverage,
    fields: &'walk mut BTreeSet<(String, String)>,
}

impl<'catalog> Walker<'_, 'catalog> {
    fn walk(&mut self, raw: Option<&Raw<'_>>, path: &str, depth: usize) -> Result<()> {
        let carrier = self.carriers.root(path)?;
        self.bound(raw, path, carrier, depth)
    }

    fn bound(
        &mut self,
        raw: Option<&Raw<'_>>,
        path: &str,
        carrier: &'catalog Value,
        depth: usize,
    ) -> Result<()> {
        if depth > MAX_RAW_DEPTH || self.coverage.checked_values >= MAX_RAW_VALUES {
            return Err(Failure::check("raw-source-walk-budget"));
        }
        self.coverage.checked_values += 1;
        self.carriers.require(carrier, path)?;
        match raw {
            None => Self::require_state(carrier, "absent"),
            Some(Raw::Null) => Self::require_state(carrier, "null"),
            Some(Raw::Struct(source)) => self.structure(source, path, carrier, depth),
            Some(Raw::Array(values)) => self.array(values, path, carrier, depth),
            Some(raw) if compare::is_reference(raw) => {
                self.reference(raw, path, carrier)?;
                self.checked_quality(path);
                Ok(())
            }
            Some(Raw::Error(error)) => {
                Err(Failure::source("sampled-source-field-unreadable", *error))
            }
            Some(raw) => {
                Self::scalar(raw, path, carrier)?;
                self.checked_quality(path);
                Ok(())
            }
        }
    }

    fn checked_quality(&mut self, path: &str) {
        if path.split('.').any(|field| {
            matches!(
                field,
                "qualityDistribution" | "qualityLocationOverride" | "qualityQuantization"
            )
        }) {
            self.coverage.quality_values += 1;
        }
    }
    fn scalar(raw: &Raw<'_>, path: &str, carrier: &Value) -> Result<()> {
        if let Some(value) = compare::present(carrier)
            && compare::scalar(raw, value, path)?
        {
            return Ok(());
        }
        Err(Failure::check("raw-scalar-value-mismatch"))
    }

    fn require_state(carrier: &Value, state: &str) -> Result<()> {
        if compare::state(carrier, state) {
            return Ok(());
        }
        Err(Failure::check("raw-source-state-mismatch"))
    }

    fn structure(
        &mut self,
        source: &StructValue<'_>,
        path: &str,
        carrier: &'catalog Value,
        depth: usize,
    ) -> Result<()> {
        if path == "defaultCargoContainers" {
            return self.cargo(source, path, carrier, depth);
        }
        if !compare::present(carrier).is_some_and(Value::is_object) {
            return Err(Failure::check("owned-structure-missing"));
        }
        let source_type = self.carriers.source_type(carrier, path)?;
        if compare::present(source_type).and_then(Value::as_str) != Some(source.type_name()) {
            return Err(Failure::check("owned-structure-type-mismatch"));
        }
        match source.type_name() {
            "ResourceTypeVolatility" => self.coverage.volatility_properties += 1,
            "ResourceTypeCraftingData" => self.coverage.quality_properties += 1,
            _ => {}
        }
        for field in source.fields() {
            self.fields
                .insert((source.type_name().to_owned(), field.name().to_owned()));
            let child = self.carriers.child(carrier, path, field.name())?;
            self.bound(
                Some(field.value()),
                &format!("{path}.{}", field.name()),
                child,
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn array(
        &mut self,
        source: &[Raw<'_>],
        path: &str,
        carrier: &'catalog Value,
        depth: usize,
    ) -> Result<()> {
        let output = array_value(carrier, source.len(), "owned-array-length-or-order")?;
        for (index, (value, emitted)) in source.iter().zip(output).enumerate() {
            self.bound(Some(value), &format!("{path}[{index}]"), emitted, depth + 1)?;
        }
        Ok(())
    }

    fn cargo(
        &mut self,
        source: &StructValue<'_>,
        path: &str,
        carrier: &'catalog Value,
        depth: usize,
    ) -> Result<()> {
        let output = array_value(carrier, source.fields().len(), "cargo-slot-cardinality")?;
        for (field, emitted) in source.fields().iter().zip(output) {
            let item = format!("{path}.{}", field.name());
            self.carriers.require(emitted, &item)?;
            let slot =
                compare::present(emitted).ok_or_else(|| Failure::check("cargo-slot-state"))?;
            let name = slot
                .get("slot")
                .ok_or_else(|| Failure::check("cargo-slot-name"))?;
            self.carriers.require(name, &format!("{item}.$fieldName"))?;
            if compare::present(name).and_then(Value::as_str) != Some(field.name()) {
                return Err(Failure::check("cargo-slot-name-order-origin"));
            }
            let entity = slot
                .get("entityClass")
                .ok_or_else(|| Failure::check("cargo-slot-entity"))?;
            self.fields
                .insert((source.type_name().to_owned(), field.name().to_owned()));
            self.coverage.packaging_slots += 1;
            self.box_status(field.value());
            self.bound(Some(field.value()), &item, entity, depth + 1)?;
        }
        Ok(())
    }

    fn reference(&self, raw: &Raw<'_>, path: &str, carrier: &Value) -> Result<()> {
        let value = compare::present(carrier)
            .filter(|value| compare::reference(raw, value))
            .ok_or_else(|| Failure::check("raw-reference-mismatch"))?;
        let guid = value
            .get("guid")
            .ok_or_else(|| Failure::check("reference-guid-origin"))?;
        self.carriers.require(guid, path)?;
        if let Raw::Reference(target) = raw {
            for (name, field) in [
                ("targetRecordIndex", "$recordIndex"),
                ("recordPath", "$recordPath"),
                ("targetType", "$type"),
            ] {
                let header = value
                    .get(name)
                    .ok_or_else(|| Failure::check("reference-header-origin"))?;
                self.carriers
                    .require_at(header, target.record_index(), field)?;
            }
        } else {
            for name in ["targetRecordIndex", "recordPath", "targetType"] {
                let header = value
                    .get(name)
                    .ok_or_else(|| Failure::check("reference-header-origin"))?;
                self.carriers.require(header, path)?;
            }
        }
        Ok(())
    }

    fn box_status(&mut self, raw: &Raw<'_>) {
        let reason = match raw {
            Raw::Null => {
                self.coverage.null_box_slots += 1;
                return;
            }
            Raw::Error(ValueError::DanglingReference { .. }) => "dangling",
            Raw::Error(ValueError::UnexpectedReferenceType { .. }) => "unexpectedTargetType",
            _ => return,
        };
        *self.coverage.unresolved_boxes.entry(reason).or_default() += 1;
    }
}

fn array_value<'a>(carrier: &'a Value, length: usize, stage: &'static str) -> Result<&'a [Value]> {
    compare::present(carrier)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .filter(|array| array.len() == length)
        .ok_or_else(|| Failure::check(stage))
}
#[cfg(test)]
#[path = "coverage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "values_mutation_tests.rs"]
mod mutation_tests;

#[cfg(test)]
#[path = "producer_e2e_tests.rs"]
mod producer_e2e_tests;

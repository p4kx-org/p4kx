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
use crate::Kind;

const ROOT_FIELDS: &[&str] = &[
    "validateDefaultCargoBox",
    "defaultCargoContainers",
    "properties",
];

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Coverage {
    checked_rows: usize,
    checked_values: usize,
    owned_fields: usize,
    packaging_slots: usize,
    volatility_properties: usize,
    quality_properties: usize,
    quality_values: usize,
    null_box_slots: usize,
    unresolved_boxes: BTreeMap<&'static str, usize>,
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
    let uses = Uses::new(catalog, origin)?;
    let mut owners = source_owners(forge)?;
    let mut coverage = Coverage::default();
    let mut fields = BTreeSet::new();
    let mut builder = TreeBuilder::new();
    for row in rows {
        watch
            .poll()
            .map_err(|error| Failure::source("archive-source-changed", error))?;
        let index = uses.record_index(row)?;
        if !owners.remove(&index) {
            return Err(Failure::check("duplicate-or-outside-resource-owner"));
        }
        let record = forge
            .record(index)
            .ok_or_else(|| Failure::check("physical-resource-record"))?;
        let tree = builder
            .build(&record)
            .map_err(|error| Failure::source("raw-resource-tree", error))?;
        identity(&tree, row)?;
        super::record_header::check(&tree, row, &uses)?;
        check_owned_fields(&tree, row, &uses, &mut coverage, &mut fields)?;
        coverage.checked_rows += 1;
    }
    if !owners.is_empty() {
        return Err(Failure::check("missing-resource-owner"));
    }
    coverage.owned_fields = fields.len();
    Ok(coverage)
}

fn source_owners(forge: &DataForge<'_>) -> Result<BTreeSet<u32>> {
    let mut owners = BTreeSet::new();
    for record in forge.records() {
        let kind = record
            .type_name()
            .map_err(|error| Failure::source("resource-source-type", error))?;
        if kind != "ResourceType" {
            continue;
        }
        let path = record
            .record_path()
            .map_err(|error| Failure::source("resource-source-path", error))?;
        let name = record
            .name()
            .map_err(|error| Failure::source("resource-source-name", error))?;
        if Kind::by_record(path, kind) == Some(Kind::ResourceTypes)
            && !crate::selection::is_kind_leftover(Kind::ResourceTypes, path, name)
        {
            owners.insert(record.index());
        }
    }
    Ok(owners)
}

fn check_owned_fields<'catalog>(
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
        walker.walk(tree.field(name), name)?;
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

struct Comparison<'tree, 'source, 'catalog> {
    raw: Option<&'tree Raw<'source>>,
    path: String,
    carrier: &'catalog Value,
    cargo_slot: Option<u64>,
}

impl<'catalog> Walker<'_, 'catalog> {
    fn walk(&mut self, raw: Option<&Raw<'_>>, path: &str) -> Result<()> {
        let carrier = self.carriers.root(path)?;
        let mut remaining = vec![Comparison {
            raw,
            path: path.to_owned(),
            carrier,
            cargo_slot: None,
        }];
        while let Some(value) = remaining.pop() {
            self.compare(value, &mut remaining)?;
        }
        Ok(())
    }

    fn compare<'tree, 'source>(
        &mut self,
        value: Comparison<'tree, 'source, 'catalog>,
        remaining: &mut Vec<Comparison<'tree, 'source, 'catalog>>,
    ) -> Result<()> {
        let Comparison {
            raw,
            path,
            carrier,
            cargo_slot,
        } = value;
        self.coverage.checked_values += 1;
        self.require_origin(carrier, &path, cargo_slot)?;
        match raw {
            None => Self::require_state(carrier, "absent"),
            Some(Raw::Null) => Self::require_state(carrier, "null"),
            Some(Raw::Struct(source)) => self.structure(source, &path, carrier, remaining),
            Some(Raw::Array(values)) => Self::array(values, &path, carrier, remaining),
            Some(raw) if compare::is_reference(raw) => {
                self.reference(raw, &path, carrier, cargo_slot)?;
                self.checked_quality(&path);
                Ok(())
            }
            Some(Raw::Error(error)) => Err(Failure::source("source-field-unreadable", *error)),
            Some(raw) => {
                Self::scalar(raw, &path, carrier)?;
                self.checked_quality(&path);
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

    fn structure<'tree, 'source>(
        &mut self,
        source: &'tree StructValue<'source>,
        path: &str,
        carrier: &'catalog Value,
        remaining: &mut Vec<Comparison<'tree, 'source, 'catalog>>,
    ) -> Result<()> {
        if path == "defaultCargoContainers" {
            return self.cargo(source, path, carrier, remaining);
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
        for field in source.fields().iter().rev() {
            self.fields
                .insert((source.type_name().to_owned(), field.name().to_owned()));
            let child = self.carriers.child(carrier, path, field.name())?;
            remaining.push(Comparison {
                raw: Some(field.value()),
                path: format!("{path}.{}", field.name()),
                carrier: child,
                cargo_slot: None,
            });
        }
        Ok(())
    }

    fn array<'tree, 'source>(
        source: &'tree [Raw<'source>],
        path: &str,
        carrier: &'catalog Value,
        remaining: &mut Vec<Comparison<'tree, 'source, 'catalog>>,
    ) -> Result<()> {
        let output = array_value(carrier, source.len(), "owned-array-length-or-order")?;
        for (index, (value, emitted)) in source.iter().zip(output).enumerate().rev() {
            remaining.push(Comparison {
                raw: Some(value),
                path: format!("{path}[{index}]"),
                carrier: emitted,
                cargo_slot: None,
            });
        }
        Ok(())
    }

    fn cargo<'tree, 'source>(
        &mut self,
        source: &'tree StructValue<'source>,
        path: &str,
        carrier: &'catalog Value,
        remaining: &mut Vec<Comparison<'tree, 'source, 'catalog>>,
    ) -> Result<()> {
        let output = array_value(carrier, source.fields().len(), "cargo-slot-cardinality")?;
        for ((field, emitted), ordinal) in source.fields().iter().zip(output).zip(0_u64..) {
            let item = format!("{path}.{}", field.name());
            self.carriers.require_cargo_slot(emitted, ordinal, &item)?;
            let slot =
                compare::present(emitted).ok_or_else(|| Failure::check("cargo-slot-state"))?;
            let name = slot
                .get("slot")
                .ok_or_else(|| Failure::check("cargo-slot-name"))?;
            self.carriers
                .require_cargo_slot(name, ordinal, &format!("{item}.$fieldName"))?;
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
            remaining.push(Comparison {
                raw: Some(field.value()),
                path: item,
                carrier: entity,
                cargo_slot: Some(ordinal),
            });
        }
        Ok(())
    }

    fn require_origin(&self, carrier: &Value, path: &str, slot: Option<u64>) -> Result<()> {
        slot.map_or_else(
            || self.carriers.require(carrier, path),
            |ordinal| self.carriers.require_cargo_slot(carrier, ordinal, path),
        )
    }

    fn reference(
        &self,
        raw: &Raw<'_>,
        path: &str,
        carrier: &Value,
        slot: Option<u64>,
    ) -> Result<()> {
        let value = compare::present(carrier)
            .filter(|value| compare::reference(raw, value))
            .ok_or_else(|| Failure::check("raw-reference-mismatch"))?;
        let guid = value
            .get("guid")
            .ok_or_else(|| Failure::check("reference-guid-origin"))?;
        self.require_origin(guid, path, slot)?;
        if let Raw::Reference(target) = raw {
            for (name, field) in [
                ("targetRecordIndex", "recordIndex"),
                ("recordPath", "fileName"),
                ("targetType", "sourceType"),
            ] {
                let header = value
                    .get(name)
                    .ok_or_else(|| Failure::check("reference-header-origin"))?;
                self.carriers
                    .require_header_at(header, target.record_index(), field)?;
            }
        } else {
            for name in ["targetRecordIndex", "recordPath", "targetType"] {
                let header = value
                    .get(name)
                    .ok_or_else(|| Failure::check("reference-header-origin"))?;
                self.require_origin(header, path, slot)?;
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

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourceLocation, SourceValueState, SourcedValue};
use serde::Serialize;
use serde_json::Value;

use crate::record_header_field::RecordHeaderField;
use crate::ship::ShipSource;

use super::super::verify::{ContributionSet, Graph};
use super::{TestResult, wire};

pub(super) struct Wire {
    header: Value,
    values: [(&'static str, Value); 17],
}

impl Wire {
    pub(super) fn read(row: &ShipSource<'_>, public: Value, protected: Value) -> TestResult<Self> {
        Ok(Self {
            header: wire(row.source())?,
            values: [
                ("name", wire(row.name())?),
                ("description", wire(row.description())?),
                ("role", wire(row.role())?),
                ("career", wire(row.career())?),
                ("crewSize", wire(row.crew_size())?),
                ("vehicleManufacturer", wire(row.vehicle_manufacturer())?),
                (
                    "attachmentManufacturer",
                    wire(row.attachment_manufacturer())?,
                ),
                ("vehicleDefinition", wire(row.vehicle_definition())?),
                ("size", wire(row.size())?),
                ("mass", wire(row.mass())?),
                ("massBasis", wire(row.mass_basis())?),
                ("hullHealth", wire(row.hull_health())?),
                ("insurance", wire(row.insurance())?),
                ("playability", wire(row.playability())?),
                ("playabilityInputs", wire(row.playability_inputs())?),
                ("publicNames", public),
                ("gameTexts", protected),
            ],
        })
    }

    pub(super) fn compare(&self, producer: &Self) -> TestResult {
        if self.header != producer.header {
            return Err("Ship SDK header differs from the scoped producer".into());
        }
        for ((name, actual), (_, expected)) in self.values.iter().zip(&producer.values) {
            if actual != expected {
                return Err(format!("Ship SDK {name} differs from the scoped producer").into());
            }
        }
        Ok(())
    }

    pub(super) fn verify(&self, graph: &Graph<'_>, index: u32) -> TestResult<usize> {
        graph.header(
            &self.header,
            "recordIndex",
            index,
            RecordHeaderField::RecordIndex,
        )?;
        graph.header(&self.header, "guid", index, RecordHeaderField::Guid)?;
        graph.header(
            &self.header,
            "sourceType",
            index,
            RecordHeaderField::SourceType,
        )?;
        let policy = ContributionSet::Retained;
        let mut count = graph.walk_cells(&self.header, policy)?;
        for (_, value) in &self.values {
            count += graph.walk_cells(value, policy)?;
        }
        Ok(count)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Metric {
    name: &'static str,
    state: SourceValueState,
    source_set_complete: bool,
    contains_encrypted_source: Option<bool>,
    immediate_uses: usize,
}

pub(super) fn metrics(
    row: &ShipSource<'_>,
    inventory: &SourceInventory,
    index: u32,
) -> TestResult<[Metric; 3]> {
    Ok([
        metric("mass", row.mass(), inventory, index)?,
        metric("massBasis", row.mass_basis(), inventory, index)?,
        metric("hullHealth", row.hull_health(), inventory, index)?,
    ])
}

fn metric<T>(
    name: &'static str,
    value: &SourcedValue<T>,
    inventory: &SourceInventory,
    index: u32,
) -> TestResult<Metric> {
    let origin = value.origin();
    for id in origin.source_uses_refs() {
        let usage = inventory
            .iter_uses()
            .find(|usage| usage.id() == id)
            .ok_or("unresolved immediate Ship metric source")?;
        match usage.location() {
            SourceLocation::DataForge { record_index, .. } if *record_index != index => {
                return Err("Ship metric refers to another physical owner".into());
            }
            SourceLocation::Subresource {
                namespace, indices, ..
            } if matches!(
                namespace.as_str(),
                "dataforgeValue" | "dataforgeRecordHeader"
            ) && indices.first().copied() != Some(u64::from(index)) =>
            {
                return Err("Ship metric original coordinate refers to another owner".into());
            }
            _ => {}
        }
    }
    Ok(Metric {
        name,
        state: value.state(),
        source_set_complete: origin.source_set_complete(),
        contains_encrypted_source: origin.contains_encrypted_source(),
        immediate_uses: origin.source_uses_refs().len(),
    })
}

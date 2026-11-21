// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, Kind};
use p4kx_extraction::{ArchiveWatch, DataForge, StructValue, Value as Original};

use super::{COMPONENT, HEAT, HEAT_FIELDS, POWER, POWER_FIELDS, TestResult};
use crate::authored_component_witness::{ComponentCandidate, select_component_owners};
use crate::real_archive_item_components as components;

pub(super) fn discover<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<[u32; 3]> {
    let mut candidates = [
        ComponentCandidate::new(forge, COMPONENT, has_named_state)?,
        ComponentCandidate::new(forge, POWER, |source| has_single(source, POWER_FIELDS))?,
        ComponentCandidate::new(forge, HEAT, |source| has_single(source, HEAT_FIELDS))?,
    ];
    let rule = Kind::Items.rule().ok_or("selected Item rule")?;
    let filter = Filter::new(components::FILTER);
    select_component_owners(forge, watch, &mut candidates, |record| {
        components::selected_item(record, &rule, &filter)
    })?;
    let [component, power, heat] = candidates;
    Ok([
        component.owner("a resource component with a named state")?,
        power.owner("a power connection with authored values")?,
        heat.owner("a heat connection with authored values")?,
    ])
}

fn has_named_state(value: &StructValue<'_>) -> bool {
    let Some(Original::Array(states)) = value.field("states") else {
        return false;
    };
    states.iter().any(|slot| {
        matches!(
            slot,
            Original::Struct(state) if matches!(state.field("name"), Some(Original::String(_)))
        )
    })
}

fn has_single(value: &StructValue<'_>, fields: &[&str]) -> bool {
    fields
        .iter()
        .any(|field| matches!(value.field(field), Some(Original::Single(_))))
}

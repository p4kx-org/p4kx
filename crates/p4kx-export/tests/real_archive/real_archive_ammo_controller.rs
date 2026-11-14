// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use serde_json::Value;

use super::{Result, check_discriminator, fields, structure};
use crate::real_archive_item_components::cell;
use crate::real_archive_item_durability::Trace;

const CONVERSIONS: &[(&str, &str)] = &[
    ("SBaseCargoUnit", "base"),
    ("SStandardCargoUnit", "standard"),
    ("SCentiCargoUnit", "centi"),
    ("SMicroCargoUnit", "micro"),
];
const CONTROLLERS: &[(&str, &str)] = &[
    ("SEntityBasePhysicsControllerParams", "base"),
    ("SEntityParticlePhysicsControllerParams", "particle"),
];
const CONTROLLER_WRAPPER: &[(&str, &str)] = &[("SEntityPhysicsControllerParams", "wrapper")];

pub(super) fn conversion<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    let Some(source) = structure(
        trace,
        original,
        row,
        "conversionRate",
        "conversionRate",
        CONVERSIONS,
    )?
    else {
        return Ok(0);
    };
    check_discriminator(source.json, source.kind)?;
    let mappings = match source.kind {
        "base" => return Ok(0),
        "standard" => &[("standardCargoUnits", "standardCargoUnits")],
        "centi" => &[("centiSCU", "centiScu")],
        "micro" => &[("microSCU", "microScu")],
        _ => return Err("unexpected verified cargo-unit shape".into()),
    };
    fields(trace, source.original, source.json, mappings)
}

pub(super) fn physics<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    let Some(wrapper) = structure(
        trace,
        original,
        row,
        "physicsControllerParams",
        "physicsControllerParams",
        CONTROLLER_WRAPPER,
    )?
    else {
        return Ok(0);
    };
    let Some(source) = structure(
        trace,
        wrapper.original,
        wrapper.json,
        "PhysType",
        "physType",
        CONTROLLERS,
    )?
    else {
        return Ok(0);
    };
    let kind = cell(source.json, "kind")?;
    check_discriminator(kind, source.kind)?;
    let mut count = fields(trace, source.original, source.json, &[("Mass", "mass")])?;
    if source.kind == "particle" {
        count += fields(
            trace,
            source.original,
            cell(kind, "value")?,
            &[
                ("pierceability", "pierceability"),
                ("radius", "radius"),
                ("thickness", "thickness"),
                ("length", "length"),
                ("airResistance", "airResistance"),
                ("disableGravity", "disableGravity"),
            ],
        )?;
    }
    Ok(count)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    InventoryVector2, InventoryVector3, ResourceCapacity, UnitSource,
};

use crate::canonical_header::originals;
use crate::canonical_header::wire::Wire;
use crate::commodity_v3_fixture::TestResult;

fn source(value: &UnitSource<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("sourceType")?.leaf(value.source_type())?;
    originals::visit_arena(value.additional_fields(), wire.field("additionalFields")?)
}

pub(super) fn vector2(value: &InventoryVector2<'_>, wire: Wire<'_>) -> TestResult {
    source(value.source(), wire)?;
    wire.field("x")?
        .cell(value.x(), |number, wire| wire.float32(number))?;
    wire.field("y")?
        .cell(value.y(), |number, wire| wire.float32(number))
}

pub(super) fn vector3(value: &InventoryVector3<'_>, wire: Wire<'_>) -> TestResult {
    source(value.source(), wire)?;
    wire.field("x")?
        .cell(value.x(), |number, wire| wire.float32(number))?;
    wire.field("y")?
        .cell(value.y(), |number, wire| wire.float32(number))?;
    wire.field("z")?
        .cell(value.z(), |number, wire| wire.float32(number))
}

pub(super) fn capacity(value: &ResourceCapacity<'_>, wire: Wire<'_>) -> TestResult {
    source(value.source(), wire)?;
    match value {
        ResourceCapacity::Base { .. } => wire.field("kind")?.raw("base"),
        ResourceCapacity::CentiCargoUnit { centi_scu, .. } => {
            wire.field("kind")?.raw("centiCargoUnit")?;
            wire.field("centiSCU")?.leaf(centi_scu)
        }
        ResourceCapacity::MicroCargoUnit { micro_scu, .. } => {
            wire.field("kind")?.raw("microCargoUnit")?;
            wire.field("microSCU")?.leaf(micro_scu)
        }
        ResourceCapacity::StandardCargoUnit {
            standard_cargo_units,
            ..
        } => {
            wire.field("kind")?.raw("standardCargoUnit")?;
            wire.field("standardCargoUnits")?
                .cell(standard_cargo_units, |number, wire| wire.float32(number))
        }
        ResourceCapacity::UnknownUnit { .. } => wire.field("kind")?.raw("unknownUnit"),
        _ => Err("a new capacity unit needs a typed traversal".into()),
    }
}

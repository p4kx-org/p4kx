// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Filter, Record, RecordTree, StructValue, Value as Original};
use serde_json::Value;

use super::TestResult;
use crate::real_archive_item_durability::Trace;

pub(super) const GRID: &str = "SCItemCargoGridParams";
const STORAGE: &str = "SCItemInventoryContainerComponentParams";
const CONTAINER: &str = "InventoryContainer";
const OPEN: &str = "InventoryOpenContainerType";
const CLOSED: &str = "InventoryClosedContainerType";
const STANDARD: &str = "SStandardCargoUnit";
const CENTI: &str = "SCentiCargoUnit";
const MICRO: &str = "SMicroCargoUnit";
const CUBIC_METRES_PER_SCU: f64 = 1.953_125;
const ROUNDING_SCALE: f64 = 10_000.0;

pub(super) struct Grid<'source> {
    pub(super) item: RecordTree<'source>,
    pub(super) container: RecordTree<'source>,
    pub(super) facility_ordinal: usize,
    pub(super) storage_ordinal: usize,
}

impl<'source> Grid<'source> {
    pub(super) fn read(
        forge: &'source DataForge<'_>,
        filter: &Filter,
        path: &str,
        mining_only: bool,
    ) -> TestResult<Self> {
        let owner = exact_record(forge, filter, path)?;
        let grid = Self::from_item(forge, owner.tree()?, mining_only)?;
        if !filter.matches(grid.container.record_path()) {
            return Err("the witness filter must admit both the Item and its Container".into());
        }
        Ok(grid)
    }

    pub(super) fn from_item(
        forge: &'source DataForge<'_>,
        item: RecordTree<'source>,
        mining_only: bool,
    ) -> TestResult<Self> {
        if item.type_name() != "EntityClassDefinition" {
            return Err("the selected Item is not a physical Entity Record".into());
        }
        let Original::Array(components) = member(item.root(), "Components")? else {
            return Err("the Item must have a readable Components array".into());
        };
        let (facility_ordinal, grid) = unique_component(components, GRID)?;
        if !matches!(grid.field("miningOnly"), Some(Original::Bool(actual)) if *actual == mining_only)
        {
            return Err(
                "the selected Item does not have the required authored miningOnly flag".into(),
            );
        }
        let (storage_ordinal, storage) = unique_component(components, STORAGE)?;
        let Original::Reference(link) = member(storage, "containerParams")? else {
            return Err("the co-owned storage must name a physical container Record".into());
        };
        let mut matches = forge.records_by_guid(link.guid());
        let target = matches
            .next()
            .ok_or("original Container reference target")?;
        if matches.next().is_some()
            || target.index() != link.record_index()
            || target.type_name()? != CONTAINER
        {
            return Err(
                "the storage Ref must identify exactly one original InventoryContainer".into(),
            );
        }
        let container = target.tree()?;
        let result = Self {
            item,
            container,
            facility_ordinal,
            storage_ordinal,
        };
        let amount = strict_operands(result.container.root())?.scu();
        if !amount.is_finite() || amount <= 0.0 {
            return Err("the selected Item needs a positive original capacity".into());
        }
        Ok(result)
    }

    pub(super) fn flag(&self, name: &str) -> TestResult<Option<&Original<'source>>> {
        Ok(self.component(self.facility_ordinal)?.field(name))
    }

    pub(super) fn components(&self) -> TestResult<&Original<'source>> {
        member(self.item.root(), "Components")
    }

    pub(super) fn reference(&self) -> TestResult<&Original<'source>> {
        member(self.component(self.storage_ordinal)?, "containerParams")
    }

    fn component(&self, ordinal: usize) -> TestResult<&StructValue<'source>> {
        let Original::Array(slots) = self.components()? else {
            return Err("the original Components array".into());
        };
        match slots.get(ordinal) {
            Some(Original::Struct(component)) => Ok(component),
            _ => Err("missing original cargo or storage slot".into()),
        }
    }
}

pub(crate) fn exact_record<'source>(
    forge: &'source DataForge<'_>,
    filter: &Filter,
    path: &str,
) -> TestResult<Record<'source>> {
    if path.is_empty() || !filter.matches(path) {
        return Err("the caller's filter must admit each exact witness Record path".into());
    }
    let mut records = forge
        .records()
        .filter(|record| record.record_path().ok() == Some(path));
    let record = records
        .next()
        .ok_or("the exact witness Record does not exist in this Build")?;
    if records.next().is_some() {
        return Err(
            "the exact witness Record path identifies more than one physical Record".into(),
        );
    }
    Ok(record)
}

fn unique_component<'read, 'source>(
    components: &'read [Original<'source>],
    source_type: &str,
) -> TestResult<(usize, &'read StructValue<'source>)> {
    let mut found = None;
    for (ordinal, value) in components.iter().enumerate() {
        match value {
            Original::Struct(component) if component.type_name() == source_type => {
                if found.replace((ordinal, component)).is_some() {
                    return Err(
                        "the Item has multiple components of one required source type".into(),
                    );
                }
            }
            Original::Struct(_) | Original::Null => {}
            _ => return Err("the original Components array contains an unreadable slot".into()),
        }
    }
    found
        .ok_or_else(|| "the selected Item needs exactly one cargo and one storage component".into())
}

pub(crate) struct Operands<'read, 'source> {
    inventory_type: &'read Original<'source>,
    input: CapacityInput<'read, 'source>,
}

enum CapacityInput<'read, 'source> {
    Open {
        declaration: &'read Original<'source>,
        axes: [&'read Original<'source>; 3],
        size: [f64; 3],
    },
    Closed {
        declaration: &'read Original<'source>,
        amount: &'read Original<'source>,
        scu: f64,
    },
}

impl<'source> Operands<'_, 'source> {
    pub(crate) fn scu(&self) -> f64 {
        let value = match self.input {
            CapacityInput::Open { size, .. } => {
                let [x, y, z] = size;
                x * y * z / CUBIC_METRES_PER_SCU
            }
            CapacityInput::Closed { scu, .. } => scu,
        };
        round(value)
    }

    pub(super) fn compare_uses(&self, trace: &Trace<'_, 'source>, scu: &Value) -> TestResult {
        trace.part(scu, self.inventory_type, "$type", "selector")?;
        match self.input {
            CapacityInput::Open {
                declaration, axes, ..
            } => {
                trace.part(scu, declaration, "$dataType", "selector")?;
                for axis in axes {
                    trace.check(scu, axis, "payload")?;
                }
            }
            CapacityInput::Closed {
                declaration,
                amount,
                ..
            } => {
                trace.part(scu, declaration, "$type", "selector")?;
                trace.check(scu, amount, "payload")?;
            }
        }
        Ok(())
    }
}

pub(crate) fn strict_operands<'read, 'source>(
    container: &'read StructValue<'source>,
) -> TestResult<Operands<'read, 'source>> {
    let inventory_type = member(container, "inventoryType")?;
    let family = structure(inventory_type)?;
    let input = match family.type_name() {
        OPEN => {
            let declaration = member(container, "interiorDimensions")?;
            let axes = coordinates(structure(declaration)?)?;
            let [
                Original::Single(x),
                Original::Single(y),
                Original::Single(z),
            ] = axes
            else {
                return Err("the open Container must store three Float32 dimensions".into());
            };
            let size = [f64::from(*x), f64::from(*y), f64::from(*z)];
            if size.iter().any(|side| !side.is_finite() || *side <= 0.0) {
                return Err("the open Container must have finite positive dimensions".into());
            }
            CapacityInput::Open {
                declaration,
                axes,
                size,
            }
        }
        CLOSED => {
            let declaration = member(family, "capacity")?;
            let unit = structure(declaration)?;
            let (field, divisor) =
                amount_member(unit.type_name()).ok_or("recognized original cargo unit")?;
            let amount = member(unit, field)?;
            let value = match (unit.type_name(), amount) {
                (STANDARD, Original::Single(value)) => f64::from(*value),
                (CENTI | MICRO, Original::Int32(value)) => f64::from(*value),
                _ => {
                    return Err(
                        "the capacity amount does not have the original supported storage".into(),
                    );
                }
            };
            CapacityInput::Closed {
                declaration,
                amount,
                scu: value / divisor,
            }
        }
        _ => {
            return Err(
                "the selected original InventoryContainer needs a concrete capacity".into(),
            );
        }
    };
    Ok(Operands {
        inventory_type,
        input,
    })
}

pub(super) fn legacy_scu(container: &StructValue<'_>) -> Option<f64> {
    let inventory = structure(container.field("inventoryType")?).ok()?;
    let amount = match inventory.type_name() {
        OPEN => {
            let dimensions = structure(container.field("interiorDimensions")?).ok()?;
            let [x, y, z] = coordinates(dimensions).ok()?.map(legacy_f32);
            let [x, y, z] = [x?, y?, z?].map(f64::from);
            if [x, y, z].into_iter().any(|axis| axis <= 0.0) {
                return None;
            }
            x * y * z / CUBIC_METRES_PER_SCU
        }
        CLOSED => {
            let unit = structure(inventory.field("capacity")?).ok()?;
            let (field, divisor) = amount_member(unit.type_name())?;
            let stored = match unit.field(field)? {
                Original::Int32(value) => f64::from(*value),
                Original::UInt32(value) => f64::from(*value),
                other => f64::from(legacy_f32(other)?),
            };
            stored / divisor
        }
        _ => return None,
    };
    (amount.is_finite() && amount >= 0.0).then(|| round(amount))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "The legacy scalar conversion narrows finite numbers before calculating capacity."
)]
fn legacy_f32(value: &Original<'_>) -> Option<f32> {
    let number = match *value {
        Original::Single(value) => value,
        Original::Double(value) => value as f32,
        Original::Int8(value) => f32::from(value),
        Original::Int16(value) => f32::from(value),
        Original::Int32(value) => value as f32,
        Original::Int64(value) => value as f32,
        Original::UInt8(value) => f32::from(value),
        Original::UInt16(value) => f32::from(value),
        Original::UInt32(value) => value as f32,
        Original::UInt64(value) => i64::try_from(value).ok()? as f32,
        _ => return None,
    };
    number.is_finite().then_some(number)
}

const fn amount_member(unit: &str) -> Option<(&'static str, f64)> {
    match unit.as_bytes() {
        b"SStandardCargoUnit" => Some(("standardCargoUnits", 1.0)),
        b"SCentiCargoUnit" => Some(("centiSCU", 100.0)),
        b"SMicroCargoUnit" => Some(("microSCU", 1_000_000.0)),
        _ => None,
    }
}

fn member<'read, 'source>(
    source: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read Original<'source>> {
    source
        .field(name)
        .ok_or_else(|| format!("missing original cargo field {name}").into())
}

fn structure<'read, 'source>(
    value: &'read Original<'source>,
) -> TestResult<&'read StructValue<'source>> {
    match value {
        Original::Struct(value) => Ok(value),
        _ => Err("original cargo Class or StrongPointer".into()),
    }
}

fn coordinates<'read, 'source>(
    size: &'read StructValue<'source>,
) -> TestResult<[&'read Original<'source>; 3]> {
    Ok([member(size, "x")?, member(size, "y")?, member(size, "z")?])
}

pub(super) fn round(value: f64) -> f64 {
    let rounded = (value * ROUNDING_SCALE).round() / ROUNDING_SCALE;
    if rounded.abs() < f64::EPSILON {
        0.0
    } else {
        rounded
    }
}

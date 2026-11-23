// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::io::Read;

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::inventory::{Encryption, Inventory, Observed, References};

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Rows {
    pub(super) sourced_cells: u64,
    pub(super) items: Option<u64>,
    pub(super) item_definitions: Option<u64>,
    pub(super) blueprint_definitions: Option<u64>,
    pub(super) container_instances: Option<u64>,
}

pub(super) fn verify(input: impl Read, inventory: &Inventory) -> serde_json::Result<Rows> {
    let mut deserializer = serde_json::Deserializer::from_reader(input);
    let mut counts = Rows::default();
    Node {
        inventory,
        counts: &mut counts,
        shape: Shape::Document,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    if [
        counts.items,
        counts.item_definitions,
        counts.blueprint_definitions,
        counts.container_instances,
    ]
    .iter()
    .any(Option::is_none)
    {
        return Err(de::Error::custom("missing declared witness table"));
    }
    Ok(counts)
}

#[derive(Clone, Copy)]
enum Table {
    Items,
    ItemDefinitions,
    BlueprintDefinitions,
    ContainerInstances,
}

impl Table {
    const fn count(self, counts: &mut Rows) -> &mut Option<u64> {
        match self {
            Self::Items => &mut counts.items,
            Self::ItemDefinitions => &mut counts.item_definitions,
            Self::BlueprintDefinitions => &mut counts.blueprint_definitions,
            Self::ContainerInstances => &mut counts.container_instances,
        }
    }
}

#[derive(Clone, Copy)]
enum Shape {
    Document,
    Definitions,
    Placements,
    Provenance,
    Variant,
    Table(Table),
    Rows(Table),
    Value,
}

impl Shape {
    fn child(self, name: &str) -> Self {
        match (self, name) {
            (Self::Document, "items") => Self::Rows(Table::Items),
            (Self::Document, "definitions") => Self::Definitions,
            (Self::Document, "placements") => Self::Placements,
            (Self::Document, "provenance") => Self::Provenance,
            (_, "variantOf") => Self::Variant,
            (Self::Definitions, "items") => Self::Table(Table::ItemDefinitions),
            (Self::Definitions, "blueprints") => Self::Table(Table::BlueprintDefinitions),
            (Self::Placements, "containerInstances") => Self::Table(Table::ContainerInstances),
            (Self::Table(table), "rows") => Self::Rows(table),
            _ => Self::Value,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Availability {
    Available,
    Absent,
    Unavailable,
}

struct Node<'audit> {
    inventory: &'audit Inventory,
    counts: &'audit mut Rows,
    shape: Shape,
}

impl<'de> DeserializeSeed<'de> for Node<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        match self.shape {
            Shape::Value | Shape::Variant => deserializer.deserialize_any(self),
            Shape::Rows(_) => deserializer.deserialize_seq(self),
            _ => deserializer.deserialize_map(self),
        }
    }
}

impl Node<'_> {
    fn finish_map<E: de::Error>(
        self,
        mut cell: Cell,
        variant: VariantReferences,
        has_availability: bool,
        rows: bool,
    ) -> Result<(), E> {
        if matches!(self.shape, Shape::Table(_)) {
            if !has_availability {
                return Err(de::Error::missing_field("availability"));
            }
            if !rows {
                return Err(de::Error::missing_field("rows"));
            }
        }
        if matches!(self.shape, Shape::Variant) {
            cell.references = variant.resolve(self.inventory).map_err(de::Error::custom)?;
        }
        if cell.check().map_err(de::Error::custom)? {
            increment(&mut self.counts.sourced_cells)?;
        }
        Ok(())
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a catalog value in its declared section shape")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut cell = Cell::default();
        let mut variant = VariantReferences::default();
        let mut availability = None;
        let mut rows = false;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "containsEncryptedSource" => set(
                    &mut cell.encrypted,
                    Encryption::from_marker(map.next_value()?),
                    "containsEncryptedSource",
                )?,
                "sourceSetComplete" => {
                    set(&mut cell.complete, map.next_value()?, "sourceSetComplete")?;
                }
                "sourceUsesRefs" if matches!(self.shape, Shape::Variant) => {
                    set(&mut variant.local, map.next_value()?, "sourceUsesRefs")?;
                }
                "sourceUseSetRef" if matches!(self.shape, Shape::Variant) => {
                    set(&mut variant.shared, map.next_value()?, "sourceUseSetRef")?;
                }
                "failedSourceUseRefs" if matches!(self.shape, Shape::Variant) => {
                    set(
                        &mut variant.attempts,
                        map.next_value()?,
                        "failedSourceUseRefs",
                    )?;
                }
                "variantSourceUseSets" if matches!(self.shape, Shape::Provenance) => {
                    map.next_value::<IgnoredAny>()?;
                }
                "sourceUsesRefs" => set(
                    &mut cell.references,
                    map.next_value_seed(References::new(self.inventory))?,
                    "sourceUsesRefs",
                )?,
                "availability" if matches!(self.shape, Shape::Table(_)) => {
                    set::<Availability, _>(&mut availability, map.next_value()?, "availability")?;
                }
                name => {
                    let shape = self.shape.child(name);
                    rows |=
                        matches!(self.shape, Shape::Table(_)) && matches!(shape, Shape::Rows(_));
                    map.next_value_seed(Node {
                        inventory: self.inventory,
                        counts: self.counts,
                        shape,
                    })?;
                }
            }
        }
        self.finish_map(cell, variant, availability.is_some(), rows)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        let mut rows = 0;
        while sequence
            .next_element_seed(Node {
                inventory: self.inventory,
                counts: self.counts,
                shape: Shape::Value,
            })?
            .is_some()
        {
            increment(&mut rows)?;
        }
        if let Shape::Rows(table) = self.shape {
            set(table.count(self.counts), rows, "catalog dataset rows")?;
        }
        Ok(())
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
}

#[derive(Default)]
struct Cell {
    encrypted: Option<Encryption>,
    complete: Option<bool>,
    references: Option<Observed>,
}

impl Cell {
    fn check(self) -> Result<bool, &'static str> {
        let complete = match (self.encrypted, self.complete, self.references) {
            (None, None, None) => return Ok(false),
            (Some(encrypted), Some(complete), Some(references)) => {
                if complete && references.has_opaque() {
                    return Err("complete source set contains an opaque coordinate");
                }
                if encrypted != references.marker(complete) {
                    return Err("source encryption marker disagrees with the contributing uses");
                }
                true
            }
            _ => return Err("incomplete sourced-value marker shape"),
        };
        Ok(complete)
    }
}

#[derive(Default)]
struct VariantReferences {
    local: Option<Vec<Box<str>>>,
    shared: Option<SharedSetRef>,
    attempts: Option<Vec<Box<str>>>,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct SharedSetRef(Option<u32>);

impl VariantReferences {
    fn resolve(self, inventory: &Inventory) -> Result<Option<Observed>, &'static str> {
        if let Some(attempts) = self.attempts {
            inventory.check_variant_attempts(&attempts)?;
        }
        match self.local {
            Some(local) => inventory
                .variant_contributors(self.shared.and_then(|shared| shared.0), &local)
                .map(Some),
            None if self.shared.is_some() => Err("variant source-use set lacks local references"),
            None => Ok(None),
        }
    }
}

fn set<T, E: de::Error>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E> {
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(field));
    }
    Ok(())
}

fn increment<E: de::Error>(count: &mut u64) -> Result<(), E> {
    *count = count
        .checked_add(1)
        .ok_or_else(|| E::custom("catalog replay count overflow"))?;
    Ok(())
}

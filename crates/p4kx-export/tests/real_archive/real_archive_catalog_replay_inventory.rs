// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub(super) struct Inventory {
    sources: u64,
    uses: HashMap<Box<str>, UseFacts>,
}

#[derive(Clone, Copy)]
pub(super) struct UseFacts {
    role: Role,
    encryption: Encryption,
    opaque: bool,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Role {
    Payload,
    Selector,
    Conversion,
    Attempted,
    Reference,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) enum Encryption {
    Encrypted,
    Unencrypted,
    Unknown,
}

#[derive(Deserialize)]
struct SourceRow {
    id: Box<str>,
    ancestors: Vec<Box<str>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UseRow {
    id: Box<str>,
    source: Box<str>,
    role: Role,
    location: Location,
    effective_encryption: Encryption,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Location {
    Opaque,
    LocalizationLookup {
        selector: Box<str>,
    },
    #[serde(other)]
    Other,
}

impl Inventory {
    pub(super) fn get(&self, id: &str) -> Option<UseFacts> {
        self.uses.get(id).copied()
    }

    pub(super) fn counts(&self) -> (u64, u64) {
        (self.sources, self.uses.len() as u64)
    }
}

impl UseFacts {
    pub(super) const fn contributes(self) -> bool {
        matches!(self.role, Role::Payload | Role::Selector | Role::Conversion)
    }

    pub(super) const fn encryption(self) -> Encryption {
        self.encryption
    }

    pub(super) const fn opaque(self) -> bool {
        self.opaque
    }
}

impl Encryption {
    pub(super) const fn from_marker(marker: Option<bool>) -> Self {
        match marker {
            Some(true) => Self::Encrypted,
            Some(false) => Self::Unencrypted,
            None => Self::Unknown,
        }
    }
}

impl<'de> Deserialize<'de> for Inventory {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(InventoryVisitor)
    }
}

struct InventoryVisitor;

impl<'de> Visitor<'de> for InventoryVisitor {
    type Value = Inventory;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the catalog provenance inventory")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Inventory, A::Error> {
        let mut sources = None;
        let mut uses = None;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "sources" if sources.is_none() => {
                    let mut index = Sources::default();
                    map.next_value_seed(FoldRows::new(|row: SourceRow| index.insert(row)))?;
                    index.validate().map_err(de::Error::custom)?;
                    sources = Some(index);
                }
                "uses" if uses.is_none() => {
                    let index = sources
                        .as_ref()
                        .ok_or_else(|| de::Error::custom("source inventory must precede uses"))?;
                    let mut builder = Uses::default();
                    map.next_value_seed(FoldRows::new(|row: UseRow| builder.insert(index, row)))?;
                    uses = Some(builder.finish().map_err(de::Error::custom)?);
                }
                "sources" | "uses" => {
                    return Err(de::Error::custom("duplicate provenance inventory"));
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(Inventory {
            sources: sources
                .ok_or_else(|| de::Error::missing_field("sources"))?
                .0
                .len() as u64,
            uses: uses.ok_or_else(|| de::Error::missing_field("uses"))?,
        })
    }
}

#[derive(Default)]
struct Sources(HashMap<Box<str>, Vec<Box<str>>>);

impl Sources {
    fn insert(&mut self, row: SourceRow) -> Result<(), &'static str> {
        if row.id.is_empty() || self.0.insert(row.id, row.ancestors).is_some() {
            return Err("invalid or duplicate physical-source ID");
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), &'static str> {
        for ancestors in self.0.values() {
            for (position, ancestor) in ancestors.iter().enumerate() {
                let parent = self
                    .0
                    .get(ancestor)
                    .ok_or("missing physical source ancestor")?;
                if ancestors.get(..position) != Some(parent.as_slice()) {
                    return Err("invalid physical source ancestry order");
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Uses {
    rows: HashMap<Box<str>, UseFacts>,
    selectors: Vec<Box<str>>,
}

impl Uses {
    fn insert(&mut self, sources: &Sources, row: UseRow) -> Result<(), &'static str> {
        if !sources.0.contains_key(&row.source) {
            return Err("source use refers to a missing physical source");
        }
        let facts = UseFacts {
            role: row.role,
            encryption: row.effective_encryption,
            opaque: matches!(row.location, Location::Opaque),
        };
        if row.id.is_empty() || self.rows.insert(row.id, facts).is_some() {
            return Err("invalid or duplicate source-use ID");
        }
        if let Location::LocalizationLookup { selector } = row.location {
            self.selectors.push(selector);
        }
        Ok(())
    }

    fn finish(self) -> Result<HashMap<Box<str>, UseFacts>, &'static str> {
        for selector in self.selectors {
            let source = self
                .rows
                .get(&selector)
                .ok_or("missing localization selector")?;
            if source.role != Role::Selector {
                return Err("localization lookup target does not have the selector role");
            }
        }
        Ok(self.rows)
    }
}

struct FoldRows<Row, F> {
    row: PhantomData<fn(Row)>,
    fold: F,
}

impl<Row, F> FoldRows<Row, F> {
    const fn new(fold: F) -> Self {
        Self {
            row: PhantomData,
            fold,
        }
    }
}

impl<'de, Row, F> DeserializeSeed<'de> for FoldRows<Row, F>
where
    Row: Deserialize<'de>,
    F: FnMut(Row) -> Result<(), &'static str>,
{
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, Row, F> Visitor<'de> for FoldRows<Row, F>
where
    Row: Deserialize<'de>,
    F: FnMut(Row) -> Result<(), &'static str>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one provenance inventory row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut sequence: A) -> Result<(), A::Error> {
        while let Some(row) = sequence.next_element::<Row>()? {
            (self.fold)(row).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}

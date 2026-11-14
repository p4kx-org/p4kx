// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io;

use p4kx_extraction::{DataForgeEntry, DataForgeOrigin, SourceLocation, StructValue, Value};
use serde_json::{Value as Json, json};

use crate::player_bundle_wire::Row;

use super::{Physical, Sources, xml};

#[path = "player_bundle_witness_weapon_tags.rs"]
mod tags;

#[path = "player_bundle_witness_weapon_loadouts.rs"]
mod loadouts;

#[derive(Default)]
pub(super) struct Coverage {
    sources: BTreeMap<usize, Weapon>,
    scopes: BTreeMap<u64, Json>,
    postings: Vec<Json>,
    loadouts: loadouts::Coverage,
}

pub(super) struct Forge<'a> {
    physical: Physical,
    source: p4kx_extraction::DataForge<'a>,
}

struct Weapon {
    record: u32,
    component: usize,
    tags: Json,
}

impl<'a> Forge<'a> {
    pub(super) fn new(source: &'a DataForgeEntry) -> io::Result<Self> {
        let DataForgeOrigin::Archive(original) = source.origin() else {
            return Err(io::Error::other("weapon DataForge origin unavailable"));
        };
        let source = source
            .open()
            .map_err(|_| io::Error::other("original weapon DataForge cannot reopen"))?;
        Ok(Self {
            physical: Physical::of(original),
            source,
        })
    }

    fn observe(&self, wire: &Json, sources: &Sources<'_>) -> io::Result<Weapon> {
        let record = u32::try_from(sourced_index(wire, "record")?).map_err(io::Error::other)?;
        let component =
            usize::try_from(sourced_index(wire, "component")?).map_err(io::Error::other)?;
        let raw = self
            .source
            .record(record)
            .ok_or_else(|| io::Error::other("missing weapon record"))?;
        if raw.type_name().ok() != Some("EntityClassDefinition") {
            return Err(io::Error::other(
                "weapon record is outside EntityClassDefinition",
            ));
        }
        let tree = raw
            .tree()
            .map_err(|_| io::Error::other("weapon source tree unreadable"))?;
        let definition = tree
            .field("Components")
            .and_then(|raw| attach_definition(raw, component))
            .ok_or_else(|| io::Error::other("original weapon component not found"))?;
        if !matches!(definition.field("Type"), Some(Value::Enum(name) | Value::String(name)) if name.eq_ignore_ascii_case("WeaponPersonal"))
        {
            return Err(io::Error::other(
                "original component is outside personal weapons",
            ));
        }
        let prefix = format!("Components[{component}]");
        let weapon = Weapon {
            record,
            component,
            tags: text(definition.field("Tags")),
        };
        for (name, path, expected) in [
            ("record", "$guid".to_owned(), json!(["present", record])),
            (
                "guid",
                "$guid".to_owned(),
                json!(["present", raw.guid().to_string()]),
            ),
            ("component", prefix.clone(), json!(["present", component])),
            (
                "itemType",
                format!("{prefix}.AttachDef.Type"),
                text(definition.field("Type")),
            ),
            (
                "subtype",
                format!("{prefix}.AttachDef.SubType"),
                text(definition.field("SubType")),
            ),
            (
                "tags",
                format!("{prefix}.AttachDef.Tags"),
                weapon.tags.clone(),
            ),
        ] {
            self.field(&wire[name], record, path, &expected, sources)?;
        }
        Ok(weapon)
    }

    fn field(
        &self,
        actual: &Json,
        record: u32,
        path: String,
        expected: &Json,
        sources: &Sources<'_>,
    ) -> io::Result<()> {
        if &json!([actual["state"], actual["value"]]) != expected {
            return Err(io::Error::other("weapon source field differs"));
        }
        sources.single_field(
            actual,
            &self.physical,
            &SourceLocation::DataForge {
                record_index: record,
                field: path,
            },
        )
    }
}

fn attach_definition<'tree, 'db>(
    value: &'tree Value<'db>,
    component: usize,
) -> Option<&'tree StructValue<'db>> {
    let Value::Array(items) = value else {
        return None;
    };
    let Value::Struct(owner) = items.get(component)? else {
        return None;
    };
    if owner.type_name() != "SAttachableComponentParams" {
        return None;
    }
    let Value::Struct(definition) = owner.field("AttachDef")? else {
        return None;
    };
    Some(definition)
}

fn text(raw: Option<&Value<'_>>) -> Json {
    match raw {
        Some(Value::Enum(value) | Value::String(value)) => json!(["present", value]),
        Some(Value::Null) => json!(["null", null]),
        Some(Value::Error(_)) => json!(["unreadable", null]),
        None => json!(["absent", null]),
        _ => json!(["invalid", null]),
    }
}

impl Coverage {
    pub(super) fn verify(
        &mut self,
        row: &Row<'_>,
        forge: &Forge<'_>,
        sources: &Sources<'_>,
    ) -> io::Result<()> {
        self.loadouts.record(row)?;
        match row {
            Row::WeaponMotionSource { index, value } => {
                let original = forge.observe(&serde_json::to_value(value)?, sources)?;
                if *index != self.sources.len() || self.sources.insert(*index, original).is_some() {
                    return Err(io::Error::other(
                        "weapon sources lose physical output order",
                    ));
                }
            }
            Row::AdbTagScope { index, value } => {
                self.scopes
                    .insert(*index as u64, serde_json::to_value(value)?);
            }
            Row::WeaponMotionTag { value } => self.postings.push(serde_json::to_value(value)?),
            _ => {}
        }
        Ok(())
    }

    pub(super) fn finish(
        &self,
        originals: &BTreeMap<u64, xml::Document>,
        forge: &Forge<'_>,
        sources: &Sources<'_>,
    ) -> io::Result<()> {
        let physical: BTreeSet<_> = self
            .sources
            .values()
            .map(|weapon| (weapon.record, weapon.component))
            .collect();
        if physical.is_empty() || physical.len() != self.sources.len() {
            return Err(io::Error::other("weapon physical source coverage differs"));
        }
        self.loadouts.verify(originals, forge, &self.sources)?;
        tags::verify(
            &self.sources,
            &self.scopes,
            &self.postings,
            originals,
            forge,
            sources,
        )
    }
}

pub(super) fn sourced_index(value: &Json, member: &str) -> io::Result<u64> {
    value[member]["value"]
        .as_u64()
        .ok_or_else(|| io::Error::other("missing original occurrence index"))
}

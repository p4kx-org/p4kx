// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use p4kx_export::{Kind, LoadoutSummary};
use p4kx_extraction::Guid;
use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, Error as _, SeqAccess, Visitor};
use serde_json::Value;

use crate::catalog_v3_consumer::{field, identity, nullable_guid, present, source_value};
use crate::real_archive_catalog_size::{artifact, replay::DigestReader};
use crate::real_archive_verification::print_line;

use super::compare_rows::Projection;
use super::{BoundMeasurement, DefinitionOwner, TestResult, measurement};

#[path = "real_archive_loadout_producer_seal.rs"]
mod producer_seal;

pub(crate) use producer_seal::ProducerSeal;

const MIN_PORTS: usize = 10_000;

impl BoundMeasurement {
    pub(crate) fn verify_loadout_census(
        &self,
        anchor: u32,
        seal: &ProducerSeal,
        verify_anchor: impl FnOnce(&Value) -> TestResult,
    ) -> TestResult {
        self.watch.check()?;
        let log = seal.producer_stdout(&self.directory)?;
        let census = RefCell::new(Census::default());
        let mut capture = measurement::normal_capture();
        for kind in [Kind::Items, Kind::Ships] {
            capture.project(
                &format!("/{}", kind.name()),
                Projection::LoadoutOwners(OwnerRows {
                    census: &census,
                    kind,
                }),
            );
        }
        capture.project(
            "/loadouts",
            Projection::LoadoutCensus(LoadoutRows {
                census: &census,
                anchor,
            }),
        );
        let catalog = artifact::read_completed(&self.directory, |input| {
            let mut sealed_input = DigestReader::new(input);
            let catalog = capture.read(&mut sealed_input)?;
            seal.check_frame(sealed_input.finish())?;
            Ok(catalog)
        })?;
        self.binding.check_catalog(&catalog)?;
        for required in ["ships", "items", "loadouts"] {
            if catalog.get(required).is_none() {
                return Err(format!("the retained Catalog has no {required} array").into());
            }
        }
        let mut census = census.into_inner();
        census.finish()?;
        if census.counts.ports < MIN_PORTS {
            return Err(format!(
                "the retained Catalog has only {} Loadout ports",
                census.counts.ports
            )
            .into());
        }
        producer_seal::check_counts(&log, census.counts)?;
        verify_anchor(
            &census
                .anchor
                .ok_or("the physical 100i Loadout is missing")?,
        )?;
        self.watch.check()?;
        Ok(())
    }
}

#[derive(Default)]
struct Census {
    ships: HashMap<u32, Guid>,
    items: HashMap<u32, Guid>,
    loadout_ships: HashMap<u32, Guid>,
    loadout_items: HashMap<u32, Guid>,
    attachments: HashMap<Guid, Option<String>>,
    counts: LoadoutSummary,
    unresolved: BTreeMap<String, usize>,
    issues: BTreeMap<String, usize>,
    anchor: Option<Value>,
}

impl Census {
    fn add_identity(&mut self, kind: Kind, row: &IdentityRow) -> TestResult {
        let record = present(&row.record_index)?
            .as_u64()
            .ok_or("missing physical owner Record index")?;
        let guid = nullable_guid(&row.guid)?.ok_or("missing owner GUID")?;
        let owners = if kind == Kind::Ships {
            &mut self.ships
        } else {
            &mut self.items
        };
        insert_owner(owners, u32::try_from(record)?, guid.parse()?)
    }

    fn add_loadout(&mut self, anchor: u32, row: Value) -> TestResult {
        let record = DefinitionOwner::Loadout.physical_index(&row)?;
        let guid = identity(&row, Kind::Loadouts)?.parse()?;
        match field(&row, Kind::Loadouts, "entityKind")?.as_str() {
            Some("ship") => insert_owner(&mut self.loadout_ships, record, guid)?,
            Some("item") => insert_owner(&mut self.loadout_items, record, guid)?,
            _ => return Err("unknown retained Loadout owner kind".into()),
        }
        let issues = row
            .get("issues")
            .and_then(Value::as_array)
            .ok_or("Loadout issue array")?;
        self.counts.with_issues += usize::from(!issues.is_empty());
        for issue in issues {
            let code = issue.as_str().ok_or("Loadout issue code")?;
            count_code(&mut self.issues, code);
        }
        self.add_ports(&row)?;
        if record == anchor && self.anchor.replace(row).is_some() {
            return Err("duplicate physical 100i Loadout".into());
        }
        Ok(())
    }

    fn add_ports(&mut self, row: &Value) -> TestResult {
        for port in row
            .get("ports")
            .and_then(Value::as_array)
            .ok_or("Loadout port array")?
        {
            self.counts.ports += 1;
            self.counts.docked_vehicles += usize::from(
                port.get("dockedVehicle")
                    .and_then(Value::as_bool)
                    .ok_or("Loadout dockedVehicle flag")?,
            );
            let guid = optional_source_text(port, "itemGuid")?;
            let class = optional_source_text(port, "itemClassName")?;
            let unresolved = port
                .get("unresolved")
                .ok_or("Loadout unresolved disposition")?;
            if let Some(code) = unresolved.as_str() {
                self.counts.unresolved += 1;
                count_code(&mut self.unresolved, code);
            } else if !unresolved.is_null() {
                return Err("Loadout unresolved disposition must be a code or null".into());
            } else if let Some(guid) = guid {
                self.counts.attached += 1;
                self.attachments
                    .entry(guid.parse()?)
                    .or_insert_with(|| class.map(str::to_owned));
            }
            self.add_ports(port)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> TestResult {
        if self.ships != self.loadout_ships {
            return Err("retained Ship and Loadout physical owners differ".into());
        }
        for (record, guid) in &self.loadout_items {
            if self.items.get(record) != Some(guid) {
                return Err(format!(
                    "item Loadout owner {record} does not match its physical Item"
                )
                .into());
            }
        }
        self.counts.ships = self.loadout_ships.len();
        self.counts.items = self.loadout_items.len();
        let known: HashSet<_> = self
            .ships
            .values()
            .chain(self.items.values())
            .copied()
            .collect();
        self.attachments.retain(|guid, _| !known.contains(guid));
        self.counts.outside = self.attachments.len();
        let mut classes = self
            .attachments
            .values()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>();
        classes.sort_unstable();
        classes.dedup();
        classes.truncate(40);
        print_line(&format!("retained Loadout census: {:?}", self.counts));
        print_line(&format!("unresolved ports by code: {:?}", self.unresolved));
        print_line(&format!(
            "loadouts with an issue, by code: {:?}",
            self.issues
        ));
        print_line(&format!(
            "attached Entities without an Item or Ship row, examples: {classes:?}"
        ));
        Ok(())
    }
}

fn insert_owner(owners: &mut HashMap<u32, Guid>, record: u32, guid: Guid) -> TestResult {
    if owners.insert(record, guid).is_some() {
        return Err(
            format!("physical Record {record} occurs more than once in this owner array").into(),
        );
    }
    Ok(())
}

fn optional_source_text<'row>(row: &'row Value, name: &str) -> TestResult<Option<&'row str>> {
    match source_value(
        row.get(name)
            .ok_or("missing Loadout attachment source value")?,
    )? {
        Some(Value::String(text)) => Ok(Some(text)),
        Some(Value::Null) | None => Ok(None),
        _ => Err("Loadout attachment source value must be text or null".into()),
    }
}

fn count_code(counts: &mut BTreeMap<String, usize>, code: &str) {
    if let Some(count) = counts.get_mut(code) {
        *count += 1;
    } else {
        counts.insert(code.to_owned(), 1);
    }
}

#[derive(Clone, Copy)]
pub(super) struct OwnerRows<'scope> {
    census: &'scope RefCell<Census>,
    kind: Kind,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityRow {
    guid: Value,
    record_index: Value,
}

impl<'de> DeserializeSeed<'de> for OwnerRows<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for OwnerRows<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a canonical Loadout owner identity array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut census = self.census.borrow_mut();
        while let Some(row) = sequence.next_element::<IdentityRow>()? {
            census
                .add_identity(self.kind, &row)
                .map_err(A::Error::custom)?;
        }
        Ok(Value::Null)
    }
}

#[derive(Clone, Copy)]
pub(super) struct LoadoutRows<'scope> {
    census: &'scope RefCell<Census>,
    anchor: u32,
}

impl<'de> DeserializeSeed<'de> for LoadoutRows<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for LoadoutRows<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the retained Loadout array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut census = self.census.borrow_mut();
        while let Some(row) = sequence.next_element::<Value>()? {
            census
                .add_loadout(self.anchor, row)
                .map_err(A::Error::custom)?;
        }
        Ok(Value::Null)
    }
}

#[test]
fn loadout_census_requires_the_physical_owner_even_for_aliases() -> TestResult {
    use serde_json::json;

    let alias = "6135a874-4cb1-4f49-9f29-5781e5991f2b";
    let cell = |value| {
        json!({"state":"present", "value":value, "sourceUsesRefs":[],
        "sourceSetComplete":true, "containsEncryptedSource":false})
    };
    let owner = |index| IdentityRow {
        guid: cell(json!(alias)),
        record_index: cell(json!(index)),
    };
    let row = |index| {
        json!({"entityGuid":cell(json!(alias)), "entityKind":cell(json!("ship")),
        "sourceRecordIndex":cell(json!(index)), "issues":[], "ports":[]})
    };
    let mut census = Census::default();
    census.add_identity(Kind::Ships, &owner(1))?;
    census.add_identity(Kind::Ships, &owner(2))?;
    census.add_loadout(1, row(1))?;
    assert!(
        census.finish().is_err(),
        "one alias cannot cover a different physical owner"
    );
    census.add_loadout(1, row(2))?;
    census.finish()?;
    assert_eq!(census.counts.ships, 2, "both physical aliases contribute");
    assert!(
        census.add_loadout(1, row(2)).is_err(),
        "a repeated physical index must fail"
    );
    Ok(())
}

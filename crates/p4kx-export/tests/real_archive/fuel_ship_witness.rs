// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_assembly::{ArchiveFiles, Attachment, LoadoutResolver};
use p4kx_extraction::{Archive, DataForge, Guid};
use serde_json::Value;

use super::{TestResult, at};
use crate::real_archive_item_durability as proof;

const SHIP: &str = "libs/foundry/records/entities/spaceships/aegs_avenger_titan";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FuelKind {
    Hydrogen,
    Quantum,
    Intake,
}

impl FuelKind {
    fn of(attach_type: &str) -> Option<Self> {
        match attach_type {
            "FuelTank" => Some(Self::Hydrogen),
            "QuantumFuelTank" => Some(Self::Quantum),
            "FuelIntake" => Some(Self::Intake),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Owner {
    pub(super) index: u32,
    pub(super) guid: Guid,
    pub(super) kind: FuelKind,
}

pub(super) struct ShipWitness<'source> {
    pub(super) record_path: &'source str,
    guid: Guid,
    placements: BTreeMap<String, Owner>,
    pub(super) owners: BTreeMap<u32, (FuelKind, &'source str)>,
}

impl<'source> ShipWitness<'source> {
    pub(super) fn select(
        archive: &Archive,
        forge: &'source DataForge<'source>,
    ) -> TestResult<Self> {
        let files = ArchiveFiles::new(archive);
        let mut resolver = LoadoutResolver::new(forge)
            .with_files(&files)
            .stopping_at_docked_vehicles();
        let record = resolver
            .entities()
            .find(SHIP)
            .map_err(|_| "select installed fuel test ship")?;
        let ship = resolver
            .resolve(&record)
            .map_err(|_| "resolve original fuel test ship")?;
        let mut placements = BTreeMap::new();
        let mut owners = BTreeMap::new();
        for (path, port) in ship.walk() {
            let Attachment::Entity(entity) = port.attachment() else {
                continue;
            };
            let Some(kind) = FuelKind::of(entity.attach_type()) else {
                continue;
            };
            let index = entity
                .record_index()
                .ok_or("original attached fuel owner index")?;
            let source = forge
                .record(index)
                .ok_or("original attached fuel owner Record")?;
            let record_path = source
                .record_path()
                .map_err(|_| "original attached fuel owner path")?;
            assert!(
                record_path == entity.record_path(),
                "loadout selects the original physical fuel path"
            );
            assert!(
                placements
                    .insert(
                        path,
                        Owner {
                            index,
                            guid: source.guid(),
                            kind
                        }
                    )
                    .is_none(),
                "fuel ports have distinct original paths"
            );
            owners.insert(index, (kind, record_path));
        }
        require_fuel_kinds(owners.values().map(|(kind, _)| *kind));
        Ok(Self {
            record_path: forge
                .record(record.index())
                .ok_or("original fuel ship Record")?
                .record_path()?,
            guid: ship.guid(),
            placements,
            owners,
        })
    }

    pub(super) fn compare(&self, catalog: &Value) -> TestResult {
        let guid = self.guid.to_string();
        let ship = proof::rows(catalog, "/loadouts")?
            .iter()
            .find(|row| row.get("entityGuid").and_then(Value::as_str) == Some(guid.as_str()))
            .ok_or("exported fuel ship loadout")?;
        assert_eq!(
            proof::text(ship, "/entityKind")?,
            "ship",
            "selected loadout is exported as a ship"
        );
        let mut stack: Vec<_> = proof::rows(ship, "/ports")?
            .iter()
            .map(|port| (String::new(), port))
            .collect();
        let mut seen = BTreeSet::new();
        while let Some((mut path, port)) = stack.pop() {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(proof::text(port, "/name")?);
            let children = proof::rows(port, "/ports")?;
            if at(port, "/dockedVehicle")?.as_bool() == Some(true) {
                assert!(
                    children.is_empty(),
                    "a docked ship contributes no nested placements"
                );
                continue;
            }
            if let Some(expected) = self.placements.get(&path) {
                assert!(
                    proof::number(
                        port,
                        "/sourceConstraints/attachedItem/targetRecordIndex/value"
                    )? == u64::from(expected.index)
                        && proof::text(port, "/sourceConstraints/attachedItem/targetGuid/value")?
                            == expected.guid.to_string(),
                    "the exported fuel port targets the original Record"
                );
                assert!(
                    seen.insert(path.clone()),
                    "the fuel port is exported only once"
                );
            }
            stack.extend(children.iter().map(|child| (path.clone(), child)));
        }
        assert!(
            seen.iter().eq(self.placements.keys()),
            "every original fuel port is exported"
        );
        crate::real_archive_verification::print_line(&format!(
            "fuel ship: {} original port paths and {} physical component owners checked",
            seen.len(),
            self.owners.len()
        ));
        Ok(())
    }
}

fn require_fuel_kinds(kinds: impl Iterator<Item = FuelKind>) {
    let kinds: BTreeSet<_> = kinds.collect();
    assert_eq!(
        kinds,
        BTreeSet::from([FuelKind::Hydrogen, FuelKind::Quantum, FuelKind::Intake]),
        "the selected original ship carries both tank types and an intake"
    );
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod compare;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Guid;

use crate::guid_text::{self, CatalogGuid};

use super::MissionPropertySet;

struct Row<'f> {
    set: MissionPropertySet<'f>,
    owners: BTreeSet<u32>,
}

/// The exact physical owner of a quarantined canonical source identity.
pub(crate) struct SetOwner<'f> {
    pub(crate) record_index: u32,
    pub(crate) guid: CatalogGuid,
    pub(crate) path: &'f str,
    pub(crate) source_type: &'f str,
    pub(crate) location: String,
}

impl<'f> SetOwner<'f> {
    fn of(record_index: u32, set: &MissionPropertySet<'f>) -> Self {
        Self {
            record_index,
            guid: set.record_guid,
            path: set.record_path,
            source_type: set.source_type,
            location: set.source_location.clone(),
        }
    }
}

/// Canonical source sets and permanently quarantined incompatible identities.
#[derive(Default)]
pub(crate) struct MissionPropertySets<'f> {
    rows: BTreeMap<Guid, Row<'f>>,
    conflicts: BTreeMap<Guid, BTreeMap<u32, SetOwner<'f>>>,
}

impl std::fmt::Debug for MissionPropertySets<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("MissionPropertySets")
            .field("rows", &self.rows.len())
            .field("conflicts", &self.conflicts.len())
            .finish()
    }
}

fn add_owners<'f>(owners: &mut BTreeMap<u32, SetOwner<'f>>, row: &Row<'f>) {
    for &index in &row.owners {
        owners
            .entry(index)
            .or_insert_with(|| SetOwner::of(index, &row.set));
    }
}

impl<'f> MissionPropertySets<'f> {
    pub(crate) fn contains(&self, guid: CatalogGuid) -> bool {
        self.rows.contains_key(&guid.0) || self.conflicts.contains_key(&guid.0)
    }

    pub(crate) fn is_ambiguous(&self, guid: CatalogGuid) -> bool {
        self.conflicts.contains_key(&guid.0)
    }

    pub(crate) fn insert(
        &mut self,
        record_index: u32,
        set: MissionPropertySet<'f>,
    ) -> Result<(), serde_json::Error> {
        self.absorb(
            set.guid.0,
            Row {
                set,
                owners: BTreeSet::from([record_index]),
            },
        )
    }

    fn absorb(&mut self, guid: Guid, mut row: Row<'f>) -> Result<(), serde_json::Error> {
        if let Some(owners) = self.conflicts.get_mut(&guid) {
            add_owners(owners, &row);
        } else if let Some(mut known) = self.rows.remove(&guid) {
            if compare::equal(&known.set, &row.set)? {
                known.owners.append(&mut row.owners);
                self.rows.insert(guid, known);
            } else {
                let mut owners = BTreeMap::new();
                add_owners(&mut owners, &known);
                add_owners(&mut owners, &row);
                self.conflicts.insert(guid, owners);
            }
        } else {
            self.rows.insert(guid, row);
        }
        Ok(())
    }

    pub(crate) fn merge(&mut self, other: Self) -> Result<(), serde_json::Error> {
        for (guid, owners) in other.conflicts {
            let target = self.conflicts.entry(guid).or_default();
            target.extend(owners);
            if let Some(row) = self.rows.remove(&guid) {
                add_owners(target, &row);
            }
        }
        for (guid, row) in other.rows {
            self.absorb(guid, row)?;
        }
        Ok(())
    }

    pub(crate) fn ambiguous_owners(&self) -> impl Iterator<Item = &SetOwner<'f>> {
        self.conflicts.values().flat_map(BTreeMap::values)
    }

    pub(crate) fn into_rows(self) -> Vec<MissionPropertySet<'f>> {
        let mut rows: Vec<_> = self.rows.into_values().map(|row| row.set).collect();
        guid_text::sort_by_guid(&mut rows, |set| set.guid);
        rows
    }
}

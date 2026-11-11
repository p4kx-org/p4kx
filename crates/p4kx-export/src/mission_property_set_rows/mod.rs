// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod compare;
#[cfg(test)]
mod tests;

use p4kx_extraction::Guid;
use std::collections::BTreeMap;

use super::MissionPropertySet;
use crate::catalog_read::original::{ExampleBodyReceipt, RecordSubject};
use crate::guid_text::{self, CatalogGuid};
use crate::{CatalogError, ExportAccountingError};

type Owners<'source> = BTreeMap<u32, SetOwner<'source>>;
struct Row<'source> {
    set: MissionPropertySet<'source>,
    owners: Owners<'source>,
}

/// One physical property-set owner and its original header and field admissions.
#[derive(Clone)]
pub(crate) struct SetOwner<'source> {
    pub(crate) record_index: u32,
    pub(crate) guid: CatalogGuid,
    pub(crate) path: &'source str,
    pub(crate) source_type: &'source str,
    pub(crate) location: String,
    subject: RecordSubject,
    body: Option<ExampleBodyReceipt>,
}

impl<'source> SetOwner<'source> {
    fn of(
        set: &MissionPropertySet<'source>,
        subject: RecordSubject,
        body: ExampleBodyReceipt,
    ) -> Result<Self, CatalogError> {
        body.check_record_owner(&subject)?;
        body.check_description(Some(&set.source_location), Some(set.source_type), None)?;
        Self::new(set, subject, Some(body))
    }

    fn new(
        set: &MissionPropertySet<'source>,
        subject: RecordSubject,
        body: Option<ExampleBodyReceipt>,
    ) -> Result<Self, CatalogError> {
        let key = subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if key.guid() != set.record_guid.0 || key.path() != Some(set.record_path) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            record_index: key.index(),
            guid: set.record_guid,
            path: set.record_path,
            source_type: set.source_type,
            location: set.source_location.clone(),
            subject,
            body,
        })
    }
    pub(crate) const fn subject(&self) -> &RecordSubject {
        &self.subject
    }
    pub(crate) fn body(&self) -> Result<&ExampleBodyReceipt, ExportAccountingError> {
        self.body
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)
    }

    fn merge(&mut self, other: Self) -> Result<(), CatalogError> {
        if self.record_index != other.record_index
            || self.guid != other.guid
            || self.path != other.path
            || self.source_type != other.source_type
            || self.location != other.location
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.subject.merge(other.subject)?;
        match (&mut self.body, other.body) {
            (Some(body), Some(other)) => body.merge(other)?,
            (None, None) => {}
            _ => return Err(ExportAccountingError::UnboundHealthSubject.into()),
        }
        Ok(())
    }
}

/// Canonical sets and quarantined aliases retaining every actual physical owner.
#[derive(Default)]
pub(crate) struct MissionPropertySets<'source> {
    rows: BTreeMap<Guid, Row<'source>>,
    conflicts: BTreeMap<Guid, Owners<'source>>,
}

impl std::fmt::Debug for MissionPropertySets<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("MissionPropertySets")
            .field("rows", &self.rows.len())
            .field("conflicts", &self.conflicts.len())
            .finish()
    }
}

fn merge_owners<'source>(
    target: &mut Owners<'source>,
    incoming: Owners<'source>,
) -> Result<(), CatalogError> {
    for (index, owner) in incoming {
        if let Some(retained) = target.get_mut(&index) {
            retained.merge(owner)?;
        } else {
            target.insert(index, owner);
        }
    }
    Ok(())
}

impl<'source> MissionPropertySets<'source> {
    pub(crate) fn contains(&self, guid: CatalogGuid) -> bool {
        self.rows.contains_key(&guid.0) || self.conflicts.contains_key(&guid.0)
    }
    pub(crate) fn is_ambiguous(&self, guid: CatalogGuid) -> bool {
        self.conflicts.contains_key(&guid.0)
    }

    #[cfg(test)]
    pub(crate) fn insert(
        &mut self,
        index: u32,
        set: MissionPropertySet<'source>,
    ) -> Result<(), CatalogError> {
        let subject = RecordSubject::accounting(
            index,
            set.record_guid.0,
            Some(crate::HealthText::new(set.record_path)),
        );
        let owner = SetOwner::new(&set, subject, None)?;
        self.insert_owner(set, owner)
    }
    pub(crate) fn insert_subject(
        &mut self,
        subject: RecordSubject,
        set: MissionPropertySet<'source>,
        body: ExampleBodyReceipt,
    ) -> Result<(), CatalogError> {
        let owner = SetOwner::of(&set, subject, body)?;
        self.insert_owner(set, owner)
    }
    fn insert_owner(
        &mut self,
        set: MissionPropertySet<'source>,
        owner: SetOwner<'source>,
    ) -> Result<(), CatalogError> {
        self.absorb(
            set.guid.0,
            Row {
                set,
                owners: BTreeMap::from([(owner.record_index, owner)]),
            },
        )
    }
    fn absorb(&mut self, guid: Guid, row: Row<'source>) -> Result<(), CatalogError> {
        if let Some(owners) = self.conflicts.get_mut(&guid) {
            merge_owners(owners, row.owners)?;
        } else if let Some(mut known) = self.rows.remove(&guid) {
            if compare::equal(&known.set, &row.set).map_err(CatalogError::Json)? {
                merge_owners(&mut known.owners, row.owners)?;
                self.rows.insert(guid, known);
            } else {
                merge_owners(&mut known.owners, row.owners)?;
                self.conflicts.insert(guid, known.owners);
            }
        } else {
            self.rows.insert(guid, row);
        }
        Ok(())
    }
    pub(crate) fn merge(&mut self, other: Self) -> Result<(), CatalogError> {
        for (guid, owners) in other.conflicts {
            let target = self.conflicts.entry(guid).or_default();
            merge_owners(target, owners)?;
            if let Some(row) = self.rows.remove(&guid) {
                merge_owners(target, row.owners)?;
            }
        }
        for (guid, row) in other.rows {
            self.absorb(guid, row)?;
        }
        Ok(())
    }
    pub(crate) fn ambiguous_owners(&self) -> impl Iterator<Item = &SetOwner<'source>> {
        self.conflicts.values().flat_map(BTreeMap::values)
    }
    pub(crate) fn into_rows(self) -> Vec<MissionPropertySet<'source>> {
        let mut rows: Vec<_> = self.rows.into_values().map(|row| row.set).collect();
        guid_text::sort_by_guid(&mut rows, |set| set.guid);
        rows
    }
}

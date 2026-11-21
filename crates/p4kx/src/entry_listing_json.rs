// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, anyhow};
use p4kx_extraction::{
    Archive, Entry, EntryDirectoryOrigins, FieldOrigins, SourceInventory, SourceUses, TextureGroup,
    TextureGroups,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::json::EntryJson;

pub(crate) struct EntryRows<'archive> {
    archive: &'archive Archive,
    selection: Selection<'archive>,
}

enum Selection<'archive> {
    Physical(&'archive [&'archive Entry]),
    Grouped(&'archive TextureGroups),
}

impl<'archive> EntryRows<'archive> {
    pub(crate) const fn physical(
        archive: &'archive Archive,
        entries: &'archive [&'archive Entry],
    ) -> Self {
        Self {
            archive,
            selection: Selection::Physical(entries),
        }
    }

    pub(crate) const fn grouped(
        archive: &'archive Archive,
        groups: &'archive TextureGroups,
    ) -> Self {
        Self {
            archive,
            selection: Selection::Grouped(groups),
        }
    }
}

impl Serialize for EntryRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        match self.selection {
            Selection::Physical(entries) => {
                for entry in entries {
                    let row =
                        EntryRow::read(self.archive, entry, None).map_err(S::Error::custom)?;
                    sequence.serialize_element(&row)?;
                }
            }
            Selection::Grouped(groups) => {
                for (index, entry) in self.archive.entries().iter().enumerate() {
                    if !groups.is_part(index) {
                        let row = EntryRow::read(self.archive, entry, groups.group_of(index))
                            .map_err(S::Error::custom)?;
                        sequence.serialize_element(&row)?;
                    }
                }
            }
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct EntryRow<'archive> {
    #[serde(flatten)]
    entry: EntryJson<'archive>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl<'archive> EntryRow<'archive> {
    fn read(
        archive: &'archive Archive,
        entry: &'archive Entry,
        group: Option<&TextureGroup>,
    ) -> Result<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let header = EntryDirectoryOrigins::observe(archive, entry, uses.clone())?;
        let mut fields = FieldOrigins::new();
        let mut entry = EntryJson::from(entry);
        if let Some(group) = group {
            let mut parts = Vec::with_capacity(group.parts().len());
            let mut paths = Vec::with_capacity(group.parts().len());
            for position in group.parts() {
                let part = archive
                    .entries()
                    .get(position)
                    .ok_or_else(|| anyhow!("missing original texture part"))?;
                parts.push(EntryDirectoryOrigins::observe(archive, part, uses.clone())?);
                paths.push(part.path());
            }
            header.write_group(&mut fields, "", entry.metadata(), &parts)?;
            entry = entry.grouped(group.compressed_size(), group.uncompressed_size(), paths);
        } else {
            header.write(&mut fields, "", entry.metadata())?;
        }
        Ok(Self {
            entry,
            field_origins: fields,
            provenance: uses.freeze()?,
        })
    }
}

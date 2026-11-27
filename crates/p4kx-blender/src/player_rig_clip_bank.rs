// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{
    DecimalU64, Entry, FieldOrigin, SourceScope, SourceValue, SourcedValue,
    animation_bank_path_crc32, archive_animation_path,
};
use p4kx_model::DbaMetadataRecords;
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};

use super::input::{Inputs, Lookup};
use super::rules::{Kind, Rule};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Bank {
    rule: usize,
    declaration: SourcedValue<u32>,
    target: SourcedValue<Lookup>,
    metadata: Option<SourcedValue<()>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct BankMatch {
    pub(super) bank: usize,
    pub(super) records: Vec<Record>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Record {
    pub(super) ordinal: usize,
    pub(super) name: SourcedValue<String>,
    metadata_range: SourcedValue<[DecimalU64; 2]>,
}

#[derive(Default)]
pub(super) struct Banks {
    pub(super) rows: Vec<Bank>,
    by_crc: BTreeMap<(usize, u32), Vec<Binding>>,
}

#[derive(Clone, Copy)]
struct Binding {
    index: usize,
    physical: Option<usize>,
}

#[derive(Default)]
pub(super) struct Cache(BTreeMap<usize, PhysicalBank>);

struct PhysicalBank {
    scope: SourceScope,
    inventory: SourcedValue<()>,
    names: BTreeMap<String, Vec<PhysicalRecord>>,
}

struct PhysicalRecord {
    ordinal: usize,
    name: String,
    name_range: Range<u64>,
    metadata_range: Range<u64>,
}

impl Banks {
    pub(super) fn read(
        inputs: &Inputs<'_, '_>,
        rules: &[Rule],
        cache: &mut Cache,
    ) -> Result<Self, PlayerSourceError> {
        let mut banks = Self::default();
        for (rule, value) in rules.iter().enumerate() {
            check(inputs.cancel)?;
            let Some(Kind::TracksDatabase { canonical }) = value.kind.value() else {
                continue;
            };
            let entries = inputs.entries(canonical)?;
            let target = inputs.lookup(&entries)?;
            let selected = if let [entry] = entries.as_slice() {
                Some((entry.index(), cache.read(inputs, entry)?))
            } else {
                None
            };
            let crc = animation_bank_path_crc32(canonical).ok_or(PlayerSourceError::Binding)?;
            banks
                .by_crc
                .entry((value.list, crc))
                .or_default()
                .push(Binding {
                    index: banks.rows.len(),
                    physical: selected.map(|(index, _)| index),
                });
            let declaration = inputs
                .sources
                .value_with_origins(SourceValue::Present(crc), &[value.kind.origin()])?;
            banks.rows.push(Bank {
                rule,
                declaration,
                target,
                metadata: selected.map(|(_, bank)| bank.inventory.clone()),
            });
        }
        Ok(banks)
    }

    pub(super) fn matches(
        &self,
        inputs: &Inputs<'_, '_>,
        cache: &Cache,
        scope: (usize, u32),
        clip: &str,
        origins: &[&FieldOrigin],
    ) -> Result<SourcedValue<Vec<BankMatch>>, PlayerSourceError> {
        let mut evidence = origins.to_vec();
        let mut matches = Vec::new();
        for Binding { index, physical } in self.by_crc.get(&scope).into_iter().flatten().copied() {
            check(inputs.cancel)?;
            let row = self.rows.get(index).ok_or(PlayerSourceError::Binding)?;
            evidence.extend([row.declaration.origin(), row.target.origin()]);
            let records = if let Some(physical) = physical {
                let bank = cache.0.get(&physical).ok_or(PlayerSourceError::Binding)?;
                evidence.push(bank.inventory.origin());
                bank.records(inputs, clip)?
            } else {
                Vec::new()
            };
            matches.push(BankMatch {
                bank: index,
                records,
            });
        }
        Ok(inputs
            .sources
            .value_with_origins(SourceValue::Present(matches), &evidence)?)
    }
}

impl Cache {
    fn read(
        &mut self,
        inputs: &Inputs<'_, '_>,
        entry: &Entry,
    ) -> Result<&PhysicalBank, PlayerSourceError> {
        use std::collections::btree_map::Entry;
        Ok(match self.0.entry(entry.index()) {
            Entry::Occupied(existing) => existing.into_mut(),
            Entry::Vacant(slot) => slot.insert(PhysicalBank::read(inputs, entry)?),
        })
    }
}

impl PhysicalBank {
    fn read(inputs: &Inputs<'_, '_>, entry: &Entry) -> Result<Self, PlayerSourceError> {
        let scope = inputs.scope(entry)?;
        let member = inputs.member(entry)?;
        let Ok(bytes) = member.payload() else {
            let inventory = inputs.unreadable(&scope, "dbaMetadataInventory")?;
            return Ok(Self {
                scope,
                inventory,
                names: BTreeMap::new(),
            });
        };
        let Ok(file) = ChunkFile::parse(bytes) else {
            let inventory = inputs.bytes(
                &scope,
                0..bytes.len() as u64,
                "dbaMetadataInventory",
                SourceValue::Invalid,
                false,
            )?;
            return Ok(Self {
                scope,
                inventory,
                names: BTreeMap::new(),
            });
        };
        let range = file.chunks_of(ivo::DBA_METADATA).next().map_or_else(
            || file.table_range(),
            |(_, chunk)| chunk.offset()..chunk.offset() + chunk.size(),
        );
        let (names, state, complete) = read_names(inputs, entry.path(), &file)?;
        let inventory = inputs.bytes(&scope, range, "dbaMetadataInventory", state, complete)?;
        Ok(Self {
            scope,
            inventory,
            names,
        })
    }

    fn records(
        &self,
        inputs: &Inputs<'_, '_>,
        clip: &str,
    ) -> Result<Vec<Record>, PlayerSourceError> {
        self.names
            .get(clip)
            .into_iter()
            .flatten()
            .map(|record| {
                let name = inputs.bytes(
                    &self.scope,
                    record.name_range.clone(),
                    "dbaClipName",
                    SourceValue::Present(record.name.clone()),
                    true,
                )?;
                let metadata_range = inputs.bytes(
                    &self.scope,
                    record.metadata_range.clone(),
                    "dbaClipMetadata",
                    SourceValue::Present([
                        DecimalU64::new(record.metadata_range.start),
                        DecimalU64::new(record.metadata_range.end),
                    ]),
                    true,
                )?;
                Ok(Record {
                    ordinal: record.ordinal,
                    name,
                    metadata_range,
                })
            })
            .collect()
    }
}

type Names = BTreeMap<String, Vec<PhysicalRecord>>;

fn read_names(
    inputs: &Inputs<'_, '_>,
    path: &str,
    file: &ChunkFile<&[u8]>,
) -> Result<(Names, SourceValue<()>, bool), PlayerSourceError> {
    let Ok(records) = DbaMetadataRecords::read(path, file) else {
        return Ok((Names::new(), SourceValue::Invalid, false));
    };
    let mut complete = true;
    let mut names = Names::new();
    for record in records {
        check(inputs.cancel)?;
        let Ok(record) = record else {
            complete = false;
            break;
        };
        let Ok(name) = std::str::from_utf8(record.name.bytes()) else {
            complete = false;
            continue;
        };
        let Some(key) = archive_animation_path(name) else {
            complete = false;
            continue;
        };
        names
            .entry(key.to_ascii_lowercase())
            .or_default()
            .push(PhysicalRecord {
                ordinal: record.ordinal,
                name: name.to_owned(),
                name_range: record.name.range(),
                metadata_range: record.metadata.range(),
            });
    }
    Ok((names, SourceValue::Present(()), complete))
}

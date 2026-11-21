// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, DataForgeError, Filter, Guid, Record};

use super::error::{self, Error, Kind as ErrorKind};

const MAX_SOURCE_IDENTITIES: usize = 1_000_000;

#[derive(Debug)]
pub(crate) struct Identity {
    pub(crate) record_index: u32,
    pub(crate) path: Result<String, DataForgeError>,
}

impl Identity {
    fn of(record: &Record<'_>) -> Self {
        Self {
            record_index: record.index(),
            path: record.record_path().map(str::to_owned),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Alias {
    pub(crate) guid: Guid,
    pub(crate) first: Identity,
    pub(crate) second: Identity,
}

impl Alias {
    fn of(first: &Record<'_>, second: &Record<'_>) -> Self {
        Self {
            guid: first.guid(),
            first: Identity::of(first),
            second: Identity::of(second),
        }
    }
}

impl fmt::Display for Alias {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source GUID {} aliases physical Records {} {:?} and {} {:?}",
            self.guid,
            self.first.record_index,
            self.first.path,
            self.second.record_index,
            self.second.path
        )
    }
}

impl std::error::Error for Alias {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.first
            .path
            .as_ref()
            .err()
            .or_else(|| self.second.path.as_ref().err())
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

pub(crate) struct ScopedRecord<'f> {
    pub(crate) record: Record<'f>,
    pub(crate) kind: Kind,
}

pub(crate) fn records<'f>(
    forge: &'f DataForge<'f>,
    kinds: &[Kind],
    filter: Option<&Filter>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Vec<ScopedRecord<'f>>> {
    let rules: Vec<_> = kinds
        .iter()
        .filter_map(|&kind| kind.rule().map(|rule| (kind, rule)))
        .collect();
    error::check(cancelled)?;
    if rules.is_empty() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    let mut identities = BTreeMap::<Guid, Record<'f>>::new();
    for record in forge.records() {
        error::check(cancelled)?;
        let type_name = record.type_name()?;
        if !rules.iter().any(|(_, rule)| rule.type_name == type_name) {
            continue;
        }
        let path = record.record_path()?;
        if filter.is_some_and(|filter| !filter.matches(path)) {
            continue;
        }
        let Some(&(kind, _)) = rules
            .iter()
            .find(|(_, rule)| rule.type_name == type_name && rule.matches_path(path))
        else {
            continue;
        };
        if identities.len() >= MAX_SOURCE_IDENTITIES {
            return Err(Error::new(
                ErrorKind::Bounds,
                "mapped source identities exceed their bound",
            )
            .into());
        }
        if let Some(first) = identities.insert(record.guid(), record) {
            return Err(Alias::of(&first, &record).into());
        }
        records.push(ScopedRecord { record, kind });
    }
    for record in forge.records() {
        error::check(cancelled)?;
        if let Some(first) = identities.get(&record.guid())
            && first.index() != record.index()
        {
            return Err(Alias::of(first, &record).into());
        }
    }
    Ok(records)
}

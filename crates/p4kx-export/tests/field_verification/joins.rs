// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Guid;
use serde_json::Value;

use super::error::{self, Error, Kind};
use super::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Multiplicity {
    Unique,
    Many,
}

pub(crate) type Index<'a> = BTreeMap<Guid, Vec<&'a Value>>;

pub(crate) fn guid(text: &str) -> super::Result<Guid> {
    let guid: Guid = text.parse()?;
    if guid.to_string() != text {
        return Err(Error::new(
            Kind::Identity,
            "GUID must use its canonical lowercase spelling",
        )
        .into());
    }
    Ok(guid)
}

pub(crate) fn index<'a>(
    rows: impl IntoIterator<Item = &'a Value>,
    identity: &Path,
    multiplicity: Multiplicity,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Index<'a>> {
    let mut index = Index::new();
    for row in rows {
        error::check(cancelled)?;
        let found = identity.select(row, cancelled)?;
        let [value] = found.as_slice() else {
            return Err(Error::new(
                Kind::Identity,
                "row does not have exactly one GUID join identity",
            )
            .into());
        };
        let guid =
            guid(value.as_str().ok_or_else(|| {
                Error::new(Kind::Identity, "join identity is not a GUID string")
            })?)?;
        let group = index.entry(guid).or_default();
        if multiplicity == Multiplicity::Unique && !group.is_empty() {
            return Err(Error::new(Kind::Identity, "duplicate unique GUID join identity").into());
        }
        group.push(row);
        if index.len() > 1_000_000 {
            return Err(Error::new(Kind::Bounds, "GUID join index exceeds its bound").into());
        }
    }
    Ok(index)
}

pub(crate) fn union(left: &Index<'_>, right: &Index<'_>) -> BTreeSet<Guid> {
    left.keys().chain(right.keys()).copied().collect()
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::Kind as CatalogKind;
use serde_json::Value;

use super::error::{self, Error, Kind};
use super::joins::{self, Index, Multiplicity};
use super::path::Path;
use crate::catalog_v3_consumer;

pub(super) fn index<'a>(
    catalog: &'a Value,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Index<'a>> {
    let owners = physical(catalog, cancelled)?;
    joins::index(
        owners.into_values(),
        &Path::fields(&["guid", "value"]),
        Multiplicity::Unique,
        cancelled,
    )
}

fn physical<'a>(
    catalog: &'a Value,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<u32, &'a Value>> {
    let mut owners = BTreeMap::new();
    for row in catalog_v3_consumer::rows(catalog, CatalogKind::Items)? {
        error::check(cancelled)?;
        let record = ordinal(row.get("recordIndex").ok_or("physical Item header")?)?;
        if owners.insert(record, row).is_some() {
            return Err(Error::new(Kind::Identity, "duplicate physical Item owner").into());
        }
    }
    Ok(owners)
}

pub(super) fn check_components(
    catalog: &Value,
    rows: &[Value],
    cancelled: &impl Fn() -> bool,
) -> super::Result<()> {
    let owners = physical(catalog, cancelled)?;
    let mut previous = BTreeMap::new();
    for row in rows {
        error::check(cancelled)?;
        let record = row
            .get("ownerRecordIndex")
            .ok_or("physical component owner")?;
        let index = ordinal(record)?;
        let owner = owners.get(&index).ok_or_else(|| {
            Error::new(
                Kind::Identity,
                "canonical component has no physical Item owner",
            )
        })?;
        if row.get("ownerGuid") != owner.get("guid") || owner.get("recordIndex") != Some(record) {
            return Err(Error::new(
                Kind::Identity,
                "canonical component differs from its original Item header",
            )
            .into());
        }
        let slot = ordinal(
            row.get("componentOrdinal")
                .ok_or("original component occurrence")?,
        )?;
        if previous
            .insert(index, slot)
            .is_some_and(|previous| previous >= slot)
        {
            return Err(Error::new(
                Kind::Identity,
                "component slots repeat or leave original order",
            )
            .into());
        }
    }
    Ok(())
}

fn ordinal(cell: &Value) -> super::Result<u32> {
    let value = catalog_v3_consumer::present(cell)?
        .as_u64()
        .ok_or_else(|| Error::new(Kind::Identity, "physical owner ordinal is not an integer"))?;
    Ok(u32::try_from(value)?)
}

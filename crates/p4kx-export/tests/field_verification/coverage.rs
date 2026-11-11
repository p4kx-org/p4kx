// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{DataForge, Guid, TreeBuilder};
use serde_json::Value;

use crate::catalog_v3_consumer;

use super::accounting::{self, Counts, Projection};
use super::audit_scope;
use super::error::{self, Error, Kind};
use super::inventory::{Disposition, FieldKey, Inventory};
use super::joins::{self, Index, Multiplicity};
use super::path::Path;
use super::schema::Schema;
use super::source::{self, Observations};
use super::source_scope;

pub(super) const SHIELD_GENERATORS: &str = "shieldGenerators";

fn observations(
    forge: &DataForge<'_>,
    schema: &Schema,
    inventory: &Inventory,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Observations> {
    let kinds = audit_scope::kinds(inventory)?;
    if kinds.iter().any(|kind| kind.rule().is_none()) {
        return Err(Error::new(
            Kind::Mapping,
            "a raw Record field survey requires an established Record-backed kind",
        )
        .into());
    }
    let records = source_scope::records(forge, &kinds, cancelled)?;
    let mut observations = Observations::new();
    let mut trees = TreeBuilder::new();
    for scoped in records {
        error::check(cancelled)?;
        let tree = trees.build(&scoped.record)?;
        source::observe(
            schema,
            scoped.record.guid(),
            tree.root(),
            &mut observations,
            cancelled,
        )?;
    }
    Ok(observations)
}

pub(super) fn projections<'a>(
    catalog: &'a Value,
    inventory: &Inventory,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<String, Index<'a>>> {
    let mut projections = BTreeMap::new();
    let kinds = audit_scope::kinds(inventory)?;
    for kind in kinds {
        let group = kind.name();
        let rows = catalog_v3_consumer::rows(catalog, kind)?;
        projections.insert(
            group.to_owned(),
            joins::index(
                rows,
                &Path::fields(
                    if matches!(
                        kind,
                        p4kx_export::Kind::Manufacturers
                            | p4kx_export::Kind::Commodities
                            | p4kx_export::Kind::ResourceTypes
                            | p4kx_export::Kind::Tags
                    ) {
                        &["guid", "value"]
                    } else {
                        &["guid"]
                    },
                ),
                Multiplicity::Unique,
                cancelled,
            )?,
        );
    }
    if inventory.fields.iter().any(|mapping| {
        matches!(&mapping.disposition, Disposition::Exported { group, .. }
            if group == SHIELD_GENERATORS)
    }) {
        projections.insert(
            SHIELD_GENERATORS.to_owned(),
            shield_projections(catalog, cancelled)?,
        );
    }
    Ok(projections)
}

fn shield_projections<'a>(
    catalog: &'a Value,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Index<'a>> {
    let rows = catalog
        .pointer("/configurations/shieldGenerators/rows")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new(Kind::Input, "missing canonical shield configuration rows"))?;
    for row in rows {
        error::check(cancelled)?;
        if row.pointer("/ownerGuid/state").and_then(Value::as_str) != Some("present") {
            return Err(Error::new(Kind::Identity, "shield owner GUID is not present").into());
        }
    }
    joins::index(
        rows,
        &Path::fields(&["ownerGuid", "value"]),
        Multiplicity::Many,
        cancelled,
    )
}

pub(crate) fn run(
    forge: &DataForge<'_>,
    inventory: &Inventory,
    catalog: &Value,
    rejected: BTreeSet<Guid>,
    exclusions: BTreeMap<Guid, String>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<FieldKey, Counts>> {
    let schema = Schema::read(forge, &inventory.roots, cancelled)?;
    let observations = observations(forge, &schema, inventory, cancelled)?;
    let groups = projections(catalog, inventory, cancelled)?;
    accounting::count_fields(
        &schema,
        &observations,
        inventory,
        &Projection {
            groups,
            rejected,
            exclusions,
        },
        cancelled,
    )
}

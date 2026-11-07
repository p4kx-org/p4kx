// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{DataForge, Guid, TreeBuilder};
use serde_json::Value;

use super::accounting::{self, Counts, Projection};
use super::audit_scope;
use super::error::{self, Error, Kind};
use super::inventory::{FieldKey, Inventory};
use super::joins::{self, Index, Multiplicity};
use super::path::Path;
use super::schema::Schema;
use super::source::{self, Observations};
use super::source_scope;

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

fn projections<'a>(
    catalog: &'a Value,
    inventory: &Inventory,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<String, Index<'a>>> {
    let mut projections = BTreeMap::new();
    let kinds = audit_scope::kinds(inventory)?;
    for kind in kinds {
        let group = kind.name();
        let rows = catalog
            .get(group)
            .and_then(Value::as_array)
            .ok_or_else(|| {
                Error::new(
                    Kind::Input,
                    "mapped catalog group is absent or not an array",
                )
            })?;
        projections.insert(
            group.to_owned(),
            joins::index(
                rows,
                &Path::fields(&["guid"]),
                Multiplicity::Unique,
                cancelled,
            )?,
        );
    }
    Ok(projections)
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
    accounting::account(
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

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use super::compare::Rule;
use super::error::{Error, Kind};
use super::published_model::{Availability, Field, Provenance, Snapshot};

fn provenance(provenance: &Provenance) -> super::Result<()> {
    let revision = &provenance.data_revision;
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new(Kind::Identity, "invalid immutable data revision").into());
    }
    for availability in [&provenance.generator_revision, &provenance.dcb_sha256] {
        let text = match availability {
            Availability::Known { value } => value,
            Availability::Unknown { reason } => reason,
        };
        if text.trim().is_empty() || text.len() > 4096 {
            return Err(Error::new(
                Kind::Identity,
                "provenance availability requires explicit bounded evidence",
            )
            .into());
        }
    }
    Ok(())
}

fn selection(snapshot: &Snapshot) -> super::Result<()> {
    let mut selected = BTreeSet::new();
    for guid in &snapshot.selection {
        if !selected.insert(super::joins::guid(guid)?) {
            return Err(Error::new(Kind::Identity, "duplicate selected GUID").into());
        }
    }
    let mut excluded = BTreeSet::new();
    for exclusion in &snapshot.exclusions {
        if !excluded.insert(super::joins::guid(&exclusion.guid)?) {
            return Err(Error::new(Kind::Identity, "duplicate exclusion identity").into());
        }
        if exclusion.reason.trim().is_empty()
            || exclusion.reason.len() > 4096
            || selected.contains(&super::joins::guid(&exclusion.guid)?)
        {
            return Err(Error::new(
                Kind::Identity,
                "exclusion needs a bounded reason and cannot also be selected",
            )
            .into());
        }
    }
    Ok(())
}

fn fields(fields: &[Field]) -> super::Result<()> {
    let mut identities = BTreeSet::new();
    for field in fields {
        if !identities.insert((field.family.group(), &field.source)) {
            return Err(Error::new(Kind::Mapping, "duplicate comparison field mapping").into());
        }
        if field.source.owner.is_empty()
            || field.source.owner.len() > 4096
            || field.source.field.is_empty()
            || field.source.field.len() > 4096
        {
            return Err(Error::new(Kind::Mapping, "invalid bounded owning field identity").into());
        }
        if let Some(join) = &field.resource_join {
            join.reference.validate()?;
            join.catalog.validate()?;
        }
        for unit in [&field.reference_unit, &field.catalog_unit] {
            if let super::units::Unit::Resource { guid, .. } = unit {
                super::joins::guid(guid)?;
            }
        }
        field.reference.validate()?;
        field.catalog.validate()?;
        let reason = match &field.rule {
            Rule::Derived { reason }
            | Rule::Curated { reason }
            | Rule::Normalized { reason, .. } => Some(reason),
            _ => None,
        };
        if reason.is_some_and(|reason| reason.trim().is_empty() || reason.len() > 4096) {
            return Err(Error::new(
                Kind::Mapping,
                "non-raw comparison rule needs a bounded reviewed reason",
            )
            .into());
        }
    }
    Ok(())
}

pub(crate) fn validate(snapshot: &Snapshot) -> super::Result<()> {
    if snapshot.schema_version != 1
        || snapshot.files.is_empty()
        || snapshot.fields.is_empty()
        || snapshot.files.len() > 10_000
        || snapshot.fields.len() > 1_000_000
        || snapshot.selection.len() > 1_000_000
        || snapshot.exclusions.len() > 1_000_000
    {
        return Err(Error::new(
            Kind::Bounds,
            "invalid snapshot version, empty scope or exceeded bounds",
        )
        .into());
    }
    provenance(&snapshot.provenance)?;
    selection(snapshot)?;
    fields(&snapshot.fields)?;
    for file in &snapshot.files {
        file.rows.validate()?;
        file.identity.validate()?;
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use super::record_targets::RecordTargets;
use crate::catalog_queries::equipment::AmmoDefinition;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_v3::ammo_definition::{
    AuthoredDamage, AuthoredProjectile, DamageChannels, ProjectileDamage,
};

pub(super) const PATH: &str = "definitions.ammoDefinitions";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<(StagedCatalogTable, RecordTargets), ReducedError> {
    parsed.with_ammo_definitions(CatalogRecordSelection::All, cancel, |snapshot| {
        let availability = snapshot.availability();
        completeness::inventory(PATH, availability, snapshot.coverage())?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        let mut targets = RecordTargets::default();
        for original in snapshot.rows() {
            targets.insert(PATH, original.source())?;
            rows.append(&Ammunition::read(original)?)?;
        }
        let table = rows.finish(availability)?;
        completeness::rows(PATH, snapshot.rows().len(), table.len())?;
        Ok((table, targets))
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Ammunition<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    projectile_params: Cell<Projectile>,
}

impl<'read> Ammunition<'read> {
    fn read(original: &'read AmmoDefinition<'_>) -> Result<Self, ReducedError> {
        let header = original.source();
        completeness::value(PATH, &header.guid)?;
        completeness::value(PATH, header.record_index())?;
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        Ok(Self {
            identity: header.into(),
            projectile_params: completeness::cell(PATH, original.projectile())?
                .try_map(Projectile::read)?,
        })
    }
}

#[derive(Serialize)]
struct Projectile {
    kind: ProjectileKind,
}

impl Projectile {
    fn read(original: &AuthoredProjectile<'_>) -> Result<Self, ReducedError> {
        let kind = match original.damage() {
            ProjectileDamage::Base => ProjectileKind::Base,
            ProjectileDamage::Bullet(damage) => ProjectileKind::Bullet {
                value: DirectDamage::read(damage)?,
            },
            ProjectileDamage::Tachyon(damage) => ProjectileKind::Tachyon {
                value: DirectDamage::read(damage)?,
            },
            ProjectileDamage::Countermeasure => ProjectileKind::Countermeasure,
        };
        Ok(Self { kind })
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ProjectileKind {
    Base,
    Bullet { value: DirectDamage },
    Tachyon { value: DirectDamage },
    Countermeasure,
}

#[derive(Serialize)]
struct DirectDamage {
    damage: Cell<Damage>,
}

impl DirectDamage {
    fn read(original: &SourcedValue<AuthoredDamage<'_>>) -> Result<Self, ReducedError> {
        Ok(Self {
            damage: completeness::cell(PATH, original)?.try_map(Damage::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Damage {
    Base,
    Info {
        value: Box<DamageChannels<Cell<Number<'static>>>>,
    },
    Parameters,
}

impl Damage {
    fn read(original: &AuthoredDamage<'_>) -> Result<Self, ReducedError> {
        Ok(match original {
            AuthoredDamage::Base => Self::Base,
            AuthoredDamage::Info { value } => Self::Info {
                value: Box::new(value.channels().try_map(|channel| {
                    completeness::cell(PATH, channel).map(|cell| cell.map(Number::from))
                })?),
            },
            AuthoredDamage::Parameters { .. } => Self::Parameters,
        })
    }
}

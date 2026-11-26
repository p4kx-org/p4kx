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
use crate::catalog_queries::equipment::component_definitions::{
    DamageResistanceMacro, DamageResistanceMacroBody, DurabilityResistanceEntry,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::resource_float::ResourceFloat32;

const PATH: &str = "definitions.damageResistanceMacros";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<(StagedCatalogTable, RecordTargets), ReducedError> {
    parsed.with_damage_resistance_macros(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability(), snapshot.coverage())?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        let mut targets = RecordTargets::default();
        for original in snapshot.rows() {
            let header = original.source();
            targets.insert(PATH, header)?;
            rows.append(&Macro::read(original)?)?;
        }
        let table = rows.finish(snapshot.availability())?;
        completeness::rows(PATH, snapshot.rows().len(), table.len())?;
        Ok((table, targets))
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Macro<'read> {
    source: RecordIdentity<'read>,
    damage_resistance: Cell<Body>,
}

impl<'read> Macro<'read> {
    fn read(original: &'read DamageResistanceMacro<'_>) -> Result<Self, ReducedError> {
        let header = original.source();
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        Ok(Self {
            source: header.into(),
            damage_resistance: completeness::cell(PATH, original.damage_resistance())?
                .try_map(Body::read)?,
        })
    }
}

#[derive(Serialize)]
struct Body {
    physical: Cell<Channel>,
    energy: Cell<Channel>,
    distortion: Cell<Channel>,
    thermal: Cell<Channel>,
    biochemical: Cell<Channel>,
    stun: Cell<Channel>,
}

impl Body {
    fn read(original: &DamageResistanceMacroBody<'_>) -> Result<Self, ReducedError> {
        let channels = original.channels();
        let channel = |original| completeness::cell(PATH, original)?.try_map(Channel::read);
        Ok(Self {
            physical: channel(channels.physical())?,
            energy: channel(channels.energy())?,
            distortion: channel(channels.distortion())?,
            thermal: channel(channels.thermal())?,
            biochemical: channel(channels.biochemical())?,
            stun: channel(channels.stun())?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Channel {
    multiplier: Cell<Number<'static>>,
    threshold: Cell<Number<'static>>,
    damage_cap: Cell<Number<'static>>,
    damage_cap_limit: Cell<Number<'static>>,
}

impl Channel {
    fn read(original: &DurabilityResistanceEntry) -> Result<Self, ReducedError> {
        Ok(Self {
            multiplier: number(original.multiplier())?,
            threshold: number(original.threshold())?,
            damage_cap: number(original.damage_cap())?,
            damage_cap_limit: number(original.damage_cap_limit())?,
        })
    }
}

fn number(source: &SourcedValue<ResourceFloat32>) -> Result<Cell<Number<'static>>, ReducedError> {
    Ok(completeness::cell(PATH, source)?.map(Number::from))
}

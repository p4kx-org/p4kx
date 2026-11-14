// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, DataForge, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use super::{
    ANIMATION_SOURCE_TYPE, ActorProceduralRecoilConfig, CLIP_SOURCE_TYPE,
    RECOIL_CONFIG_SOURCE_TYPE, RECOIL_MODIFIERS_SOURCE_TYPE, WeaponProceduralAnimation,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_weapon_actions::WeaponActions;
use crate::catalog_v3::referenced_definition_read::{self, Definitions, Projection, Selection};
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_ledger::ExportLedger;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, CatalogReadFailure};

/// Procedural weapon animation and actor recoil configuration Records.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponProceduralTables<'source> {
    weapon_procedural_animations: CatalogTable<WeaponProceduralAnimation<'source>>,
    actor_procedural_recoil_configs: CatalogTable<ActorProceduralRecoilConfig<'source>>,
    weapon_procedural_clips: CatalogTable<WeaponProceduralTarget<'source>>,
    actor_procedural_recoil_modifiers: CatalogTable<WeaponProceduralTarget<'source>>,
}

/// Original body of a referenced clip or actor recoil modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct WeaponProceduralTarget<'source> {
    source: CanonicalRecordHeader<'source>,
    fields: SourceArenaOutput<'source>,
}

#[derive(Default)]
pub(in crate::catalog_v3) struct WeaponProceduralEvidence<'source> {
    pub(in crate::catalog_v3) numbers: Vec<CapturedNumber>,
    pub(in crate::catalog_v3) failures: Vec<CatalogReadFailure<'source>>,
    pub(in crate::catalog_v3) ledger: ExportLedger,
}

impl<'source> WeaponProceduralEvidence<'source> {
    fn rows<Row>(&mut self, result: Definitions<'source, Row>) -> Vec<Row> {
        self.numbers.extend(result.numbers);
        self.failures.extend(result.failures);
        self.ledger.merge(result.ledger);
        result.rows.into_iter().map(|(_, row)| row).collect()
    }
}

pub(in crate::catalog_v3) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    weapons: &[WeaponActions<'source>],
) -> Result<
    (
        WeaponProceduralTables<'source>,
        WeaponProceduralEvidence<'source>,
    ),
    CatalogError,
> {
    let reader = Reader {
        forge,
        input,
        filter,
        cancel,
    };
    let mut evidence = WeaponProceduralEvidence::default();
    let animations = evidence.rows(
        reader.read(
            ANIMATION_SOURCE_TYPE,
            &weapons
                .iter()
                .filter_map(WeaponActions::animation_index)
                .collect(),
            |_, source, fields, notes| WeaponProceduralAnimation::read(source, fields, notes),
        )?,
    );
    let recoil = evidence.rows(
        reader.read(
            RECOIL_CONFIG_SOURCE_TYPE,
            &weapons
                .iter()
                .filter_map(WeaponActions::actor_recoil_index)
                .collect(),
            |_, source, fields, notes| ActorProceduralRecoilConfig::read(source, fields, notes),
        )?,
    );
    let weapon_procedural_clips = reader.targets(
        CLIP_SOURCE_TYPE,
        animations
            .iter()
            .flat_map(WeaponProceduralAnimation::linked_definitions),
        &mut evidence,
    )?;
    let actor_procedural_recoil_modifiers = reader.targets(
        RECOIL_MODIFIERS_SOURCE_TYPE,
        recoil
            .iter()
            .flat_map(ActorProceduralRecoilConfig::linked_definitions),
        &mut evidence,
    )?;
    Ok((
        WeaponProceduralTables {
            weapon_procedural_animations: CatalogTable::new(
                forge,
                &[ANIMATION_SOURCE_TYPE],
                animations,
            ),
            actor_procedural_recoil_configs: CatalogTable::new(
                forge,
                &[RECOIL_CONFIG_SOURCE_TYPE],
                recoil,
            ),
            weapon_procedural_clips,
            actor_procedural_recoil_modifiers,
        },
        evidence,
    ))
}

struct Reader<'read, 'source> {
    forge: &'source DataForge<'source>,
    input: &'read OriginalCatalogInput<'read, 'source>,
    filter: Option<&'read Filter>,
    cancel: Option<&'read CancelToken>,
}

impl<'source> Reader<'_, 'source> {
    fn targets(
        &self,
        source_type: &'static str,
        linked: impl Iterator<Item = u32>,
        evidence: &mut WeaponProceduralEvidence<'source>,
    ) -> Result<CatalogTable<WeaponProceduralTarget<'source>>, CatalogError> {
        let rows = evidence.rows(self.read(source_type, &linked.collect(), original_target)?);
        Ok(CatalogTable::new(self.forge, &[source_type], rows))
    }

    fn read<Row>(
        &self,
        source_type: &'static str,
        linked: &BTreeSet<u32>,
        project: Projection<'source, Row>,
    ) -> Result<Definitions<'source, Row>, CatalogError> {
        referenced_definition_read::read(
            self.forge,
            self.input,
            &Selection {
                source_types: &[source_type],
                linked,
                filter: self.filter,
                inspection: ConfigurationInspection::WeaponProceduralDefinition,
            },
            self.cancel,
            project,
        )
    }
}

fn original_target<'source>(
    _source_type: &'source str,
    source: CanonicalRecordHeader<'source>,
    fields: Observed<'_, '_, '_, 'source>,
    _notes: &mut Vec<FieldError>,
) -> Result<WeaponProceduralTarget<'source>, CatalogError> {
    Ok(WeaponProceduralTarget {
        source,
        fields: fields.authored_members()?,
    })
}

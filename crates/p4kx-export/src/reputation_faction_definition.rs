// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, DecimalU64, Guid, RecordTree, SourceValueState, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::faction_definition::FactionText;
use super::reputation_blackboard::BlackboardProperty;
use super::reputation_relation::{self, ReputationRelation};
use super::reputation_trigger_configuration::ReputationTriggerConfiguration;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::GameTextRef;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "FactionReputation";
const CONTEXT: &str = "SReputationContextUI";
const PERK_LIST: &str = "SPerkReputationListParams";

type Link<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type LinkList<'source> = SourcedValue<Vec<Link<'source>>>;
type ProjectedField = (&'static str, SourceValueState);

/// An original faction reputation Record with its presentation and relationship fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FactionReputationDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    presentation: FactionReputationPresentation<'source>,
    #[serde(flatten)]
    relations: FactionReputationRelations<'source>,
    properties_bb: SourcedValue<Vec<SourcedValue<BlackboardProperty<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> FactionReputationDefinition<'source> {
    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        sandbox_state: SourceValueState,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let presentation = FactionReputationPresentation::read(fields, projection, notes)?;
        let relations = FactionReputationRelations::read(fields, projection, notes)?;
        let properties_bb = BlackboardProperty::read_array(fields, projection, notes)?;
        let authored_fields = fields.authored_members_except(
            presentation
                .projected()
                .into_iter()
                .chain(relations.projected())
                .chain([
                    ("propertiesBB", properties_bb.state()),
                    ("sandboxTriggers", sandbox_state),
                ]),
        )?;
        Ok(Self {
            source,
            presentation,
            relations,
            properties_bb,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct FactionReputationPresentation<'source> {
    name: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    display_name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<FactionText<'source, GameTextRef>>,
    #[schemars(extend("x-gameText" = true))]
    background: SourcedValue<&'source str>,
    geid: SourcedValue<DecimalU64>,
    logo: SourcedValue<&'source str>,
    mission_board_logo: SourcedValue<&'source str>,
    guild_banner: SourcedValue<&'source str>,
    hide_in_delpih_app: SourcedValue<bool>,
    is_npc: SourcedValue<bool>,
}

impl<'source> FactionReputationPresentation<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: fields.borrowed_string("name", notes)?,
            display_name: fields.protected_locale_or_legacy_key(
                "displayName",
                projection,
                notes,
            )?,
            description: FactionText::read(fields, "description", notes, |fact| {
                projection.protected_locale_fact(fact)
            })?,
            background: fields.borrowed_string("background", notes)?,
            geid: fields.uint64("GEID", notes)?,
            logo: fields.borrowed_string("logo", notes)?,
            mission_board_logo: fields.borrowed_string("missionBoardLogo", notes)?,
            guild_banner: fields.borrowed_string("guildBanner", notes)?,
            hide_in_delpih_app: fields.boolean("hideInDelpihApp", notes)?,
            is_npc: fields.boolean("isNPC", notes)?,
        })
    }

    const fn projected(&self) -> [ProjectedField; 10] {
        [
            ("name", self.name.state()),
            ("displayName", self.display_name.state()),
            ("description", self.description.state()),
            ("background", self.background.state()),
            ("GEID", self.geid.state()),
            ("logo", self.logo.state()),
            ("missionBoardLogo", self.mission_board_logo.state()),
            ("guildBanner", self.guild_banner.state()),
            ("hideInDelpihApp", self.hide_in_delpih_app.state()),
            ("isNPC", self.is_npc.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct FactionReputationRelations<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    reputation_context_properties_ui: Link<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    allies: LinkList<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    enemies: LinkList<'source>,
    hostility_params: SourcedValue<ReputationRelation<'source>>,
    allied_params: SourcedValue<ReputationRelation<'source>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    perk_reputation_reward_list: LinkList<'source>,
}

impl<'source> FactionReputationRelations<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let relation = |name, notes: &mut Vec<FieldError>| {
            fields.declared_class_to(
                name,
                reputation_relation::SOURCE_TYPE,
                notes,
                |relation, notes| ReputationRelation::read(relation, projection, notes),
            )
        };
        Ok(Self {
            reputation_context_properties_ui: fields.record_link_to(
                "reputationContextPropertiesUI",
                CONTEXT,
                notes,
            )?,
            allies: fields.record_link_array_to("allies", SOURCE_TYPE, notes)?,
            enemies: fields.record_link_array_to("enemies", SOURCE_TYPE, notes)?,
            hostility_params: relation("hostilityParams", notes)?,
            allied_params: relation("alliedParams", notes)?,
            perk_reputation_reward_list: fields.record_link_array_to(
                "perkReputationRewardList",
                PERK_LIST,
                notes,
            )?,
        })
    }

    const fn projected(&self) -> [ProjectedField; 6] {
        [
            (
                "reputationContextPropertiesUI",
                self.reputation_context_properties_ui.state(),
            ),
            ("allies", self.allies.state()),
            ("enemies", self.enemies.state()),
            ("hostilityParams", self.hostility_params.state()),
            ("alliedParams", self.allied_params.state()),
            (
                "perkReputationRewardList",
                self.perk_reputation_reward_list.state(),
            ),
        ]
    }
}

/// A prepared definition and its physical owner and numeric receipts.
pub(crate) struct PreparedFactionReputation<'source> {
    definition: FactionReputationDefinition<'source>,
    triggers: ReputationTriggerConfiguration<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct FactionReputationRead<'source> {
    pub(crate) value: PreparedFactionReputation<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one original faction reputation definition without following its target records.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<FactionReputationRead<'source>, CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let triggers =
        ReputationTriggerConfiguration::read(tree, fields, projection.origins(), &mut notes)?;
    let definition = FactionReputationDefinition::read(
        CanonicalRecordHeader::read(tree, projection.origins())?,
        fields,
        projection,
        triggers.state(),
        &mut notes,
    )?;
    Ok(FactionReputationRead {
        value: PreparedFactionReputation {
            definition,
            triggers,
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

/// Physical faction definitions and their owned sandbox configurations.
pub(crate) struct FactionReputationTables<'source> {
    pub(crate) definitions: CatalogTable<FactionReputationDefinition<'source>>,
    pub(crate) triggers: CatalogTable<ReputationTriggerConfiguration<'source>>,
}

/// Builds both tables from the same admitted owner Records.
pub(crate) fn tables<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedFactionReputation<'source>>,
) -> FactionReputationTables<'source> {
    let (definitions, triggers) = rows
        .into_iter()
        .map(|row| (row.definition, row.triggers))
        .unzip();
    FactionReputationTables {
        definitions: CatalogTable::new(forge, &[SOURCE_TYPE], definitions),
        triggers: CatalogTable::new(forge, &[SOURCE_TYPE], triggers),
    }
}

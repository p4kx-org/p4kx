// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::infraction_definition::{InfractionUse, SET_TYPE};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::PublicNameRef;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(crate) const SOURCE_TYPE: &str = "Jurisdiction";
const ENTITY_TYPE: &str = "EntityClassDefinition";
const RESOURCE_TYPE: &str = "ResourceType";
const JOURNAL_TYPE: &str = "JournalEntry";
const SUBSTANCE_TYPE: &str = "ControlledSubstanceClass";
const IMPOUND_TYPE: &str = "ImpoundingDefinition";
const IMPOUND_TRIGGER: &str = "EImpoundingTrigger";

/// A physical jurisdiction and the law facts authored directly on it.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct JurisdictionDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    subsumption_jurisdiction: SourcedValue<&'source str>,
    name: SourcedValue<PublicNameRef>,
    logo_path: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    parent_jurisdiction: SourcedValue<Box<SourceRecordReference<'source>>>,
    respects_parent_jurisdiction_laws: SourcedValue<bool>,
    infractions: SourcedValue<Vec<SourcedValue<InfractionUse<'source>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    infraction_sets: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    journal_entry: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(flatten)]
    possession: JurisdictionPossession<'source>,
    base_fine: SourcedValue<i32>,
    early_payment_period: SourcedValue<ResourceFloat32>,
    is_prison: SourcedValue<bool>,
    impounding_definitions: SourcedValue<Vec<SourcedValue<ImpoundingDefinition<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> JurisdictionDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let subsumption_jurisdiction = fields.borrowed_string("subsumptionJurisdiction", notes)?;
        let name = fields.public_name("name", projection, notes)?;
        let logo_path = fields.borrowed_string("logoPath", notes)?;
        let parent_jurisdiction =
            fields.record_link_to("parentJurisdiction", SOURCE_TYPE, notes)?;
        let respects_parent_jurisdiction_laws =
            fields.boolean("respectsParentJurisdictionLaws", notes)?;
        let infractions = InfractionUse::read_array(fields, projection, notes)?;
        let infraction_sets = fields.record_link_array_to("infractionSets", SET_TYPE, notes)?;
        let journal_entry = fields.record_link_to("journalEntry", JOURNAL_TYPE, notes)?;
        let possession = JurisdictionPossession::read(fields, notes)?;
        let base_fine = fields.int32("baseFine", notes)?;
        let early_payment_period = fields.single("earlyPaymentPeriod", notes)?;
        let is_prison = fields.boolean("isPrison", notes)?;
        let impounding_definitions = fields.class_array_to(
            "impoundingDefinitions",
            IMPOUND_TYPE,
            notes,
            |owned, notes| ImpoundingDefinition::read(owned, projection, notes),
        )?;
        let authored_fields =
            fields.authored_members_except(possession.projected().into_iter().chain([
                ("subsumptionJurisdiction", subsumption_jurisdiction.state()),
                ("name", name.state()),
                ("logoPath", logo_path.state()),
                ("parentJurisdiction", parent_jurisdiction.state()),
                (
                    "respectsParentJurisdictionLaws",
                    respects_parent_jurisdiction_laws.state(),
                ),
                ("infractions", infractions.state()),
                ("infractionSets", infraction_sets.state()),
                ("journalEntry", journal_entry.state()),
                ("baseFine", base_fine.state()),
                ("earlyPaymentPeriod", early_payment_period.state()),
                ("isPrison", is_prison.state()),
                ("impoundingDefinitions", impounding_definitions.state()),
            ]))?;
        Ok(Self {
            source,
            subsumption_jurisdiction,
            name,
            logo_path,
            parent_jurisdiction,
            respects_parent_jurisdiction_laws,
            infractions,
            infraction_sets,
            journal_entry,
            possession,
            base_fine,
            early_payment_period,
            is_prison,
            impounding_definitions,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JurisdictionPossession<'source> {
    max_stolen_goods_possession_scu: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    prohibited_goods: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    prohibited_resources: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    controlled_substance_classes:
        SourcedValue<Vec<SourcedValue<ControlledSubstanceClass<'source>>>>,
}

impl<'source> JurisdictionPossession<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let max_stolen_goods_possession_scu =
            fields.single("maxStolenGoodsPossessionSCU", notes)?;
        let prohibited_goods =
            fields.record_link_array_to("prohibitedGoods", ENTITY_TYPE, notes)?;
        let prohibited_resources =
            fields.record_link_array_to("prohibitedResources", RESOURCE_TYPE, notes)?;
        let controlled_substance_classes = fields.class_array_to(
            "controlledSubstanceClasses",
            SUBSTANCE_TYPE,
            notes,
            ControlledSubstanceClass::read,
        )?;
        Ok(Self {
            max_stolen_goods_possession_scu,
            prohibited_goods,
            prohibited_resources,
            controlled_substance_classes,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 4] {
        [
            (
                "maxStolenGoodsPossessionSCU",
                self.max_stolen_goods_possession_scu.state(),
            ),
            ("prohibitedGoods", self.prohibited_goods.state()),
            ("prohibitedResources", self.prohibited_resources.state()),
            (
                "controlledSubstanceClasses",
                self.controlled_substance_classes.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ControlledSubstanceClass<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    commodities: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    resources: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    max_possession_scu: SourcedValue<ResourceFloat32>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ControlledSubstanceClass<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let commodities = fields.record_link_array_to("commodities", ENTITY_TYPE, notes)?;
        let resources = fields.record_link_array_to("resources", RESOURCE_TYPE, notes)?;
        let max_possession_scu = fields.single("maxPossessionSCU", notes)?;
        let authored_fields = fields.authored_members_except([
            ("commodities", commodities.state()),
            ("resources", resources.state()),
            ("maxPossessionSCU", max_possession_scu.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            commodities,
            resources,
            max_possession_scu,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ImpoundingDefinition<'source> {
    source_type: SourcedValue<&'source str>,
    name: SourcedValue<PublicNameRef>,
    trigger: SourcedValue<&'source str>,
    impounding_time_seconds: SourcedValue<ResourceFloat32>,
    impounding_fine_uec: SourcedValue<i32>,
    ignore_if_against_party_member: SourcedValue<bool>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> ImpoundingDefinition<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = fields.public_name("name", projection, notes)?;
        let trigger = fields.borrowed_enum_to("trigger", IMPOUND_TRIGGER, notes)?;
        let impounding_time_seconds = fields.single("impoundingTimeSeconds", notes)?;
        let impounding_fine_uec = fields.int32("impoundingFineUEC", notes)?;
        let ignore_if_against_party_member = fields.boolean("ignoreIfAgainstPartyMember", notes)?;
        let authored_fields = fields.authored_members_except([
            ("name", name.state()),
            ("trigger", trigger.state()),
            ("impoundingTimeSeconds", impounding_time_seconds.state()),
            ("impoundingFineUEC", impounding_fine_uec.state()),
            (
                "ignoreIfAgainstPartyMember",
                ignore_if_against_party_member.state(),
            ),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            name,
            trigger,
            impounding_time_seconds,
            impounding_fine_uec,
            ignore_if_against_party_member,
            authored_fields,
        })
    }
}

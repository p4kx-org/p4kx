// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "Faction";
const REPUTATION: &str = "FactionReputation";
const REACTION: &str = "ReactionType";
const TYPE: &str = "FactionType";
const OVERRIDE: &str = "FriendlyFireReactionOverride";

type Link<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// Authored faction behaviour, legal authority and independent reputation links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FactionDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    name: SourcedValue<FactionText<'source, PublicNameRef>>,
    #[schemars(extend("x-gameText" = true))]
    description: SourcedValue<FactionText<'source, GameTextRef>>,
    default_reaction: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    allied_factions: SourcedValue<Vec<Link<'source>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    enemy_factions: SourcedValue<Vec<Link<'source>>>,
    friendly_fire_behavior_overrides:
        SourcedValue<Vec<SourcedValue<FriendlyFireOverride<'source>>>>,
    faction_type: SourcedValue<&'source str>,
    able_to_arrest: SourcedValue<bool>,
    polices_lawful_trespass: SourcedValue<bool>,
    polices_criminality: SourcedValue<bool>,
    no_legal_rights: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction_reputation_ref: Link<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

/// One locale key in its canonical domain or a directly stored string.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(rename = "FactionTextOf{Reference}", transform = every_field_required)]
enum FactionText<'source, Reference> {
    Localized { reference: Reference },
    Literal { text: &'source str },
}

impl<'source, Reference> FactionText<'source, Reference> {
    fn read<'scope, 'read>(
        fields: Observed<'_, 'scope, 'read, 'source>,
        name: &str,
        notes: &mut Vec<FieldError>,
        register: impl FnOnce(
            &OriginalValueFact<'scope, 'read, 'source>,
        ) -> Result<Reference, CatalogError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.locale_or_string(
            name,
            notes,
            |fact| register(fact).map(|reference| Self::Localized { reference }),
            |text| Self::Literal { text },
        )
    }
}

/// One ordered original override and any additional owned members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct FriendlyFireOverride<'source> {
    source_type: SourcedValue<&'source str>,
    reaction_type: SourcedValue<&'source str>,
    should_allow_friendly_fire: SourcedValue<bool>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> FriendlyFireOverride<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let reaction_type = fields.borrowed_enum_to("reactionType", REACTION, notes)?;
        let should_allow_friendly_fire = fields.boolean("shouldAllowFriendlyFire", notes)?;
        let authored_fields = fields.authored_members_except([
            ("reactionType", reaction_type.state()),
            (
                "shouldAllowFriendlyFire",
                should_allow_friendly_fire.state(),
            ),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            reaction_type,
            should_allow_friendly_fire,
            authored_fields,
        })
    }
}

/// Prepared Faction and its physical accounting identity.
pub(crate) struct PreparedFaction<'source> {
    definition: FactionDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_path: &'source str,
    pub(crate) physical_index: u32,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct FactionRead<'source> {
    pub(crate) value: PreparedFaction<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one selected Faction from its exact owned fields without following target bodies.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<FactionRead<'source>, CatalogError> {
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
    let definition = FactionDefinition::read(
        CanonicalRecordHeader::read(tree, projection.origins())?,
        fields,
        projection,
        &mut notes,
    )?;
    Ok(FactionRead {
        value: PreparedFaction {
            definition,
            physical_guid: tree.guid(),
            physical_path: tree.record_path(),
            physical_index: tree.record_index(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

impl<'source> FactionDefinition<'source> {
    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = FactionText::read(fields, "name", notes, |fact| {
            projection.public_name_fact(fact)
        })?;
        let description = FactionText::read(fields, "description", notes, |fact| {
            projection.protected_locale_fact(fact)
        })?;
        let default_reaction = fields.borrowed_enum_to("defaultReaction", REACTION, notes)?;
        let allied_factions = fields.record_link_array_to("alliedFactions", SOURCE_TYPE, notes)?;
        let enemy_factions = fields.record_link_array_to("enemyFactions", SOURCE_TYPE, notes)?;
        let friendly_fire_behavior_overrides = fields.class_array_to(
            "friendlyFireBehaviorOverrides",
            OVERRIDE,
            notes,
            FriendlyFireOverride::read,
        )?;
        let faction_type = fields.borrowed_enum_to("factionType", TYPE, notes)?;
        let able_to_arrest = fields.boolean("ableToArrest", notes)?;
        let polices_lawful_trespass = fields.boolean("policesLawfulTrespass", notes)?;
        let polices_criminality = fields.boolean("policesCriminality", notes)?;
        let no_legal_rights = fields.boolean("noLegalRights", notes)?;
        let faction_reputation_ref =
            fields.record_link_to("factionReputationRef", REPUTATION, notes)?;
        let authored_fields = fields.authored_members_except([
            ("name", name.state()),
            ("description", description.state()),
            ("defaultReaction", default_reaction.state()),
            ("alliedFactions", allied_factions.state()),
            ("enemyFactions", enemy_factions.state()),
            (
                "friendlyFireBehaviorOverrides",
                friendly_fire_behavior_overrides.state(),
            ),
            ("factionType", faction_type.state()),
            ("ableToArrest", able_to_arrest.state()),
            ("policesLawfulTrespass", polices_lawful_trespass.state()),
            ("policesCriminality", polices_criminality.state()),
            ("noLegalRights", no_legal_rights.state()),
            ("factionReputationRef", faction_reputation_ref.state()),
        ])?;
        Ok(Self {
            source,
            name,
            description,
            default_reaction,
            allied_factions,
            enemy_factions,
            friendly_fire_behavior_overrides,
            faction_type,
            able_to_arrest,
            polices_lawful_trespass,
            polices_criminality,
            no_legal_rights,
            faction_reputation_ref,
            authored_fields,
        })
    }
}

/// Builds the Faction table with its independent physical source-family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedFaction<'source>>,
) -> CatalogTable<FactionDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}

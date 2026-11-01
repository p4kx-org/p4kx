// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::mission_layers::one;
use crate::record_fields::{boolean, integer, linked, structs, text};
use crate::spawn_model::TagTerm;
use crate::spawn_read::Reader;
use crate::template_issues::{TemplateError, Unknowns};

const MODIFIER_PREFIX: &str = "MissionModifier_";

/// The fields every modifier shares.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ModifierHeader {
    /// The `modifierName`, `null` when empty.
    pub(crate) modifier_name: Option<String>,
    /// The `enabled` flag.
    pub(crate) enabled: Option<bool>,
    /// The `missionVariableName` of the property the `locationProperty` points to.
    pub(crate) location_property: Option<String>,
}

/// How a faction reacts to the other in a `FactionHostility` modifier. A value the catalog does
/// not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum Reaction {
    /// Friendly.
    Friendly,
    /// Neutral.
    Neutral,
    /// Hostile.
    Hostile,
}

impl Reaction {
    /// Returns the reaction the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Friendly" => Self::Friendly,
            "Neutral" => Self::Neutral,
            "Hostile" => Self::Hostile,
            _ => return None,
        })
    }
}

/// The licence a `LawLicense` modifier asks for. A value the catalog does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum LicenseType {
    /// `IllegalGoodsLicense_ClassA`.
    IllegalGoodsLicenseClassA,
    /// `IllegalGoodsLicense_ClassB`.
    IllegalGoodsLicenseClassB,
    /// `IllegalGoodsLicense_ClassC`.
    IllegalGoodsLicenseClassC,
}

impl LicenseType {
    /// Returns the licence the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "IllegalGoodsLicense_ClassA" => Self::IllegalGoodsLicenseClassA,
            "IllegalGoodsLicense_ClassB" => Self::IllegalGoodsLicenseClassB,
            "IllegalGoodsLicense_ClassC" => Self::IllegalGoodsLicenseClassC,
            _ => return None,
        })
    }
}

/// A modifier of a template or a broker entry (`MissionModifier_*`), tagged by `modifier`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "modifier", rename_all = "camelCase")]
pub(crate) enum TemplateModifier {
    /// `MissionModifier_SecurityClearance`.
    #[serde(rename_all = "camelCase")]
    SecurityClearance {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `clearanceToken` Record.
        clearance_token_guid: Option<CatalogGuid>,
    },
    /// `MissionModifier_RequestRentShip`.
    #[serde(rename_all = "camelCase")]
    RequestRentShip {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `itemRecordGUID`, the ship item that is rented; an entry of `items` or `ships`.
        item_guid: Option<CatalogGuid>,
        /// The `durationSeconds` of the rental.
        duration_seconds: Option<i64>,
        /// The `clearRentalOnFail` flag.
        clear_rental_on_fail: Option<bool>,
    },
    /// `MissionModifier_ShipKioskFilter`.
    #[serde(rename_all = "camelCase")]
    ShipKioskFilter {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `terms` of the `kioskFilterType`: a ship matches one of them.
        terms: Vec<TagTerm>,
    },
    /// `MissionModifier_FactionHostility`.
    #[serde(rename_all = "camelCase")]
    FactionHostility {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `faction` Record, a `Faction`; no kind lists these.
        faction_guid: Option<CatalogGuid>,
        /// The `myReaction`: the reaction of the mission giver's faction to `faction`.
        my_reaction: Option<Reaction>,
        /// The `theirReaction`: the reaction of `faction` to the mission giver's faction.
        their_reaction: Option<Reaction>,
        /// The `ignoreCriminalHostility` flag.
        ignore_criminal_hostility: Option<bool>,
    },
    /// `MissionModifier_HostileMission`.
    #[serde(rename_all = "camelCase")]
    HostileMission {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `missionBrokerEntry` of the hostile mission, an entry of `missionBrokerEntries`.
        broker_entry_guid: Option<CatalogGuid>,
        /// The `legalToAttack` flag.
        legal_to_attack: Option<bool>,
    },
    /// `MissionModifier_IgnoreMissionPlayerCriminality`.
    #[serde(rename_all = "camelCase")]
    IgnoreMissionPlayerCriminality {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `missionBrokerEntry` whose players' criminality is ignored, an entry of
        /// `missionBrokerEntries`.
        broker_entry_guid: Option<CatalogGuid>,
    },
    /// `MissionModifier_LawLicense`.
    #[serde(rename_all = "camelCase")]
    LawLicense {
        /// The fields every modifier has.
        #[serde(flatten)]
        header: ModifierHeader,
        /// The `licenseType`.
        license_type: Option<LicenseType>,
    },
}

/// Reads the `modifiers` of `root`.
///
/// # Errors
///
/// Returns [`TemplateError::UnknownStruct`] for a modifier of a struct the catalog does not know.
pub(crate) fn modifiers(
    reader: &Reader<'_, '_>,
    unknowns: &Unknowns,
    root: Option<&StructValue<'_>>,
) -> Result<Vec<TemplateModifier>, TemplateError> {
    structs(root, "modifiers")
        .map(|modifier| read(reader, unknowns, modifier))
        .collect()
}

fn read(
    reader: &Reader<'_, '_>,
    unknowns: &Unknowns,
    modifier: &StructValue<'_>,
) -> Result<TemplateModifier, TemplateError> {
    let parent = Some(modifier);
    let header = ModifierHeader {
        modifier_name: text(parent, "modifierName").map(str::to_owned),
        enabled: boolean(parent, "enabled"),
        location_property: reader.property_variable(parent, "locationProperty"),
    };
    let reaction = |name: &str| unknowns.parse("reaction", text(parent, name), Reaction::parse);
    let name = modifier.type_name();
    Ok(match name.strip_prefix(MODIFIER_PREFIX).unwrap_or(name) {
        "SecurityClearance" => TemplateModifier::SecurityClearance {
            header,
            clearance_token_guid: linked(parent, "clearanceToken"),
        },
        "RequestRentShip" => TemplateModifier::RequestRentShip {
            header,
            item_guid: linked(parent, "itemRecordGUID"),
            duration_seconds: integer(parent, "durationSeconds"),
            clear_rental_on_fail: boolean(parent, "clearRentalOnFail"),
        },
        "ShipKioskFilter" => TemplateModifier::ShipKioskFilter {
            header,
            terms: kiosk_terms(reader, parent),
        },
        "FactionHostility" => TemplateModifier::FactionHostility {
            header,
            faction_guid: linked(parent, "faction"),
            my_reaction: reaction("myReaction"),
            their_reaction: reaction("theirReaction"),
            ignore_criminal_hostility: boolean(parent, "ignoreCriminalHostility"),
        },
        "HostileMission" => TemplateModifier::HostileMission {
            header,
            broker_entry_guid: linked(parent, "missionBrokerEntry"),
            legal_to_attack: boolean(parent, "legalToAttack"),
        },
        "IgnoreMissionPlayerCriminality" => TemplateModifier::IgnoreMissionPlayerCriminality {
            header,
            broker_entry_guid: linked(parent, "missionBrokerEntry"),
        },
        "LawLicense" => TemplateModifier::LawLicense {
            header,
            license_type: unknowns.parse(
                "license type",
                text(parent, "licenseType"),
                LicenseType::parse,
            ),
        },
        _ => return Err(TemplateError::unknown("modifiers", modifier)),
    })
}

fn kiosk_terms(reader: &Reader<'_, '_>, modifier: Option<&StructValue<'_>>) -> Vec<TagTerm> {
    structs(one(modifier, "kioskFilterType"), "terms")
        .map(|term| TagTerm {
            positive_tags: reader.tag_array(Some(term), "positiveTags"),
            negative_tags: reader.tag_array(Some(term), "negativeTags"),
        })
        .collect()
}

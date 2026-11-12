// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, StructValue, Value};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::mission_text_inputs::{BrokerTexts, OrganizationTexts, TemplateTexts};

use crate::mission_hauling;
use crate::mission_layers;
use crate::record_fields;
use crate::record_read::ReadError;
use crate::spawn_layers::Level;
use crate::spawn_pools::SpawnPools;

use super::super::{Broker, MissionTypeName, Piece, RewardDependency, Template, TemplateFacts};
use super::selection::Action;

pub(super) const FACTION_DISPLAY_NAME: &str = "displayName";

pub(super) struct TextInputs<'fields, 'scope, 'read, 'catalog, 'source> {
    pub(super) fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    pub(super) origins: &'fields CatalogOrigins,
    pub(super) tags: Option<&'fields CapturedTags<'catalog, 'source>>,
}

pub(super) fn project<'f>(
    action: Action,
    tree: &RecordTree<'f>,
    texts: Option<TextInputs<'_, '_, '_, '_, 'f>>,
    spawns: impl FnOnce(&mut SpawnPools<'f>) -> Result<Level, ReadError>,
    reward: impl FnOnce() -> Result<RewardDependency, ReadError>,
) -> Result<Option<Piece<'f>>, ReadError> {
    let root = Some(tree.root());
    let guid = tree.guid();
    Ok(match action {
        Action::Template => {
            let mut pools = SpawnPools::default();
            Some(Piece::Template(
                guid,
                template_of(tree, texts, &mut pools, spawns)?,
                pools,
            ))
        }
        Action::Type => Some(Piece::Type(
            guid,
            MissionTypeName {
                name_key: mission_layers::key(root, "LocalisedTypeName"),
            },
        )),
        Action::Organization => Some(Piece::Organization(
            guid,
            texts
                .map(|input| {
                    OrganizationTexts::read(
                        input.fields,
                        input.origins,
                        input.tags.ok_or(crate::CatalogError::MissionTextPlan)?,
                    )
                })
                .transpose()?,
        )),
        Action::Locality => Some(Piece::Locality(guid, available_locations(root))),
        Action::Broker => Some(Piece::Broker(
            guid,
            Broker {
                texts: texts
                    .map(|input| BrokerTexts::read(input.fields, input.origins))
                    .transpose()?,
                mission_type: record_fields::reference(root, "type").map(|guid| guid.0),
                location: record_fields::reference(root, "locationMissionAvailable")
                    .map(|guid| guid.0),
                objective_orders: mission_hauling::objective_orders(root),
            },
        )),
        Action::Faction => Some(Piece::Faction(
            guid,
            mission_layers::key(root, FACTION_DISPLAY_NAME),
        )),
        Action::Reward => Some(Piece::Reward(guid, tree.record_index(), reward()?)),
    })
}

fn template_of<'f>(
    tree: &RecordTree<'f>,
    texts: Option<TextInputs<'_, '_, '_, '_, 'f>>,
    pools: &mut SpawnPools<'f>,
    read_spawns: impl FnOnce(&mut SpawnPools<'f>) -> Result<Level, ReadError>,
) -> Result<Template<'f>, ReadError> {
    let root = Some(tree.root());
    let display = mission_layers::one(root, "contractDisplayInfo");
    let spawns = read_spawns(pools)?;
    let facts = texts
        .as_ref()
        .map(|input| TemplateFacts::read(tree, input.fields, input.origins).map(Box::new))
        .transpose()?;
    Ok(Template {
        texts: texts
            .map(|input| TemplateTexts::read(input.fields, input.origins))
            .transpose()?,
        mission_type: record_fields::reference(display, "type").map(|guid| guid.0),
        hauling: mission_hauling::property_orders(root, "contractProperties"),
        objective_orders: mission_hauling::objective_orders(root),
        facts,
        spawns: Some(spawns),
    })
}

fn available_locations(locality: Option<&StructValue<'_>>) -> Vec<Guid> {
    mission_layers::items(locality, "availableLocations")
        .iter()
        .filter_map(|location| match location {
            Value::Reference(target) if target.location().is_none() => Some(target.guid()),
            _ => None,
        })
        .collect()
}

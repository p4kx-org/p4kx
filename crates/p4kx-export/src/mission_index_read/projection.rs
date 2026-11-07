// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, RecordTree, StructValue, TagTree, Value};

use crate::blueprint;
use crate::mission_hauling;
use crate::mission_layers;
use crate::record_fields;
use crate::spawn_error::SpawnError;
use crate::spawn_layers::Level;
use crate::spawn_model::SpawnLevel;
use crate::spawn_pools::SpawnPools;
use crate::spawn_read::Reader;

use super::super::{Broker, MissionTypeName, Piece, Template, Variants};
use super::selection::Action;

pub(super) fn project<'f>(
    action: Action,
    tree: &RecordTree<'f>,
    tags: &TagTree,
) -> Result<Option<Piece<'f>>, SpawnError> {
    let root = Some(tree.root());
    let guid = tree.guid();
    Ok(match action {
        Action::Template => {
            let mut pools = SpawnPools::default();
            Some(Piece::Template(
                guid,
                template_of(tree, tags, &mut pools)?,
                pools,
            ))
        }
        Action::Type => Some(Piece::Type(
            guid,
            MissionTypeName {
                name_key: mission_layers::key(root, "LocalisedTypeName"),
            },
        )),
        Action::Organization => Some(Piece::Organization(guid, organization_variants(root))),
        Action::Locality => Some(Piece::Locality(guid, available_locations(root))),
        Action::Broker => Some(Piece::Broker(
            guid,
            Broker {
                title: mission_layers::key(root, "title"),
                description: mission_layers::key(root, "description"),
                mission_type: record_fields::reference(root, "type").map(|guid| guid.0),
                location: record_fields::reference(root, "locationMissionAvailable")
                    .map(|guid| guid.0),
                objective_orders: mission_hauling::objective_orders(root),
            },
        )),
        Action::Faction => Some(Piece::Faction(
            guid,
            mission_layers::key(root, "displayName"),
        )),
        Action::Reward => record_fields::integer(root, "reputationAmount")
            .map(|amount| Piece::Reward(guid, amount)),
    })
}

fn template_of<'f>(
    tree: &RecordTree<'f>,
    tags: &TagTree,
    pools: &mut SpawnPools<'f>,
) -> Result<Template<'f>, SpawnError> {
    let root = Some(tree.root());
    let display = mission_layers::one(root, "contractDisplayInfo");
    let reader = Reader::new(tags, tree.root());
    let spawns = Level::read(
        SpawnLevel::Template,
        &reader,
        (root, "contractProperties"),
        (None, ""),
        pools,
    )?;
    Ok(Template {
        title: title_key(display),
        mission_type: record_fields::reference(display, "type").map(|guid| guid.0),
        properties: mission_layers::properties(root, "contractProperties"),
        hauling: mission_hauling::property_orders(root, "contractProperties"),
        objective_orders: mission_hauling::objective_orders(root),
        illegal: record_fields::boolean(display, "illegal"),
        can_be_shared: mission_layers::items(
            mission_layers::one(root, "contractClass"),
            "additionalParams",
        )
        .iter()
        .filter_map(blueprint::as_struct)
        .find_map(|settings| record_fields::boolean(Some(settings), "canBeShared")),
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

/// Returns the localisation key of the first display string of `display`, a `ContractDisplayInfo`.
pub(crate) fn title_key<'f>(display: Option<&StructValue<'f>>) -> Option<&'f str> {
    match mission_layers::items(display, "displayString").first()? {
        Value::Struct(entry) => mission_layers::key(Some(entry), "value"),
        Value::Locale(text) | Value::String(text) => Some(*text)
            .filter(|text| text.starts_with('@') && !p4kx_extraction::is_placeholder_key(text)),
        _ => None,
    }
}

fn organization_variants<'f>(organization: Option<&StructValue<'f>>) -> Variants<'f> {
    let container = mission_layers::one(organization, "stringVariants");
    mission_layers::items(container, "variants")
        .iter()
        .filter_map(blueprint::as_struct)
        .filter_map(|variant| {
            let tag = record_fields::reference(Some(variant), "tag")?;
            Some((tag.0, mission_layers::key(Some(variant), "string")?))
        })
        .collect()
}

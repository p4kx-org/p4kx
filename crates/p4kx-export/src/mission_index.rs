// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    CancelToken, DataForge, Guid, Record, RecordTree, StructValue, TagTree, TreeBuilder, Value,
    is_placeholder_key,
};
use rayon::prelude::*;

use crate::blueprint::as_struct;
use crate::contract_hauling::HaulingRequirement;
use crate::mission_hauling::{HaulingProperty, objective_orders, property_orders};
use crate::mission_layers::{Property, items, key, one, properties};
use crate::record_fields::{boolean, integer, reference};
use crate::spawn_error::SpawnError;
use crate::spawn_layers::Level;
use crate::spawn_model::SpawnLevel;
use crate::spawn_pools::SpawnPools;
use crate::spawn_read::Reader;
use crate::{CatalogError, CatalogFailure};

const TEMPLATES: &str = "libs/foundry/records/contracts/contracttemplates/";
const TEMPLATE_TYPE: &str = "ContractTemplate";
const TYPES: &str = "libs/foundry/records/missiontype/";
const ORGANIZATIONS: &str = "libs/foundry/records/missiondata/pu_organizations/";
const REWARDS: &str = "libs/foundry/records/reputation/rewards/missionrewards_reputation/";
const LOCALITIES: &str = "libs/foundry/records/missiondata/pu_missionlocality/";
const BROKERS: &str = "libs/foundry/records/missionbroker/";
const FACTIONS: &str = "libs/foundry/records/factions/factionreputation/";

/// A `MissionBrokerEntry` Record: what a contract without a template states about itself.
#[derive(Debug, Default)]
pub(crate) struct Broker<'f> {
    pub(crate) title: Option<&'f str>,
    pub(crate) description: Option<&'f str>,
    pub(crate) mission_type: Option<Guid>,
    pub(crate) location: Option<Guid>,
    pub(crate) objective_orders: Vec<HaulingRequirement>,
}

/// What a `ContractTemplate` Record contributes to a contract: the title, the mission type and the
/// `illegal` flag of its display info, its properties and the `canBeShared` flag of its contract
/// class.
#[derive(Debug, Default)]
pub(crate) struct Template<'f> {
    pub(crate) title: Option<&'f str>,
    pub(crate) mission_type: Option<Guid>,
    pub(crate) properties: Vec<Property<'f>>,
    pub(crate) hauling: Vec<HaulingProperty<'f>>,
    pub(crate) objective_orders: Vec<HaulingRequirement>,
    pub(crate) illegal: Option<bool>,
    pub(crate) can_be_shared: Option<bool>,
    /// The ship spawns and location queries of the template.
    pub(crate) spawns: Option<Level>,
}

/// A `MissionType` Record: the key of its localised name.
#[derive(Debug)]
pub(crate) struct MissionTypeName<'f> {
    pub(crate) name_key: Option<&'f str>,
}

/// The string variants of a mission organisation: the Tag Record and the localisation key of each.
pub(crate) type Variants<'f> = Vec<(Guid, &'f str)>;

enum Piece<'f> {
    Template(Guid, Template<'f>, SpawnPools<'f>),
    Failed(CatalogFailure),
    Type(Guid, MissionTypeName<'f>),
    Organization(Guid, Variants<'f>),
    Reward(Guid, i64),
    Locality(Guid, Vec<Guid>),
    Broker(Guid, Broker<'f>),
    Faction(Guid, Option<&'f str>),
}

/// The Records a contract refers to, read once: templates, mission types, mission
/// organisations, reputation rewards, mission localities, broker entries, faction reputations
/// and the tag tree.
pub(crate) struct MissionIndex<'f> {
    templates: HashMap<Guid, Template<'f>>,
    types: HashMap<Guid, MissionTypeName<'f>>,
    organizations: HashMap<Guid, Variants<'f>>,
    rewards: HashMap<Guid, i64>,
    localities: HashMap<Guid, Vec<Guid>>,
    brokers: HashMap<Guid, Broker<'f>>,
    factions: HashMap<Guid, Option<&'f str>>,
    tags: TagTree,
    pools: SpawnPools<'f>,
    failures: Vec<CatalogFailure>,
}

impl<'f> MissionIndex<'f> {
    /// Reads the Records of `forge` a contract refers to; a Record that cannot be read is left
    /// out, so what refers to it resolves to nothing.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
    pub(crate) fn read(
        forge: &'f DataForge<'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let records: Vec<Record<'f>> = forge.records().collect();
        let tags = TagTree::build(forge);
        let pieces: Vec<Piece<'f>> = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Err(CatalogError::Cancelled);
                }
                Ok(piece(trees, record, &tags).ok().flatten())
            })
            .filter_map(Result::transpose)
            .collect::<Result<_, _>>()?;
        let mut index = Self {
            templates: HashMap::new(),
            types: HashMap::new(),
            organizations: HashMap::new(),
            rewards: HashMap::new(),
            localities: HashMap::new(),
            brokers: HashMap::new(),
            factions: HashMap::new(),
            tags,
            pools: SpawnPools::default(),
            failures: Vec::new(),
        };
        for piece in pieces {
            match piece {
                Piece::Template(guid, template, found) => {
                    index.templates.insert(guid, template);
                    index.pools.merge(found);
                }
                Piece::Failed(failure) => index.failures.push(failure),
                Piece::Type(guid, kind) => {
                    index.types.insert(guid, kind);
                }
                Piece::Organization(guid, variants) => {
                    index.organizations.insert(guid, variants);
                }
                Piece::Reward(guid, amount) => {
                    index.rewards.insert(guid, amount);
                }
                Piece::Locality(guid, locations) => {
                    index.localities.insert(guid, locations);
                }
                Piece::Broker(guid, broker) => {
                    index.brokers.insert(guid, broker);
                }
                Piece::Faction(guid, name) => {
                    index.factions.insert(guid, name);
                }
            }
        }
        Ok(index)
    }

    /// Takes the spawn descriptions and location queries of the templates and the templates that
    /// could not be read.
    pub(crate) fn take_template_results(&mut self) -> (SpawnPools<'f>, Vec<CatalogFailure>) {
        (
            std::mem::take(&mut self.pools),
            std::mem::take(&mut self.failures),
        )
    }

    /// Returns the tag tree of the DataForge.
    pub(crate) const fn tags(&self) -> &TagTree {
        &self.tags
    }

    pub(crate) fn template(&self, guid: Guid) -> Option<&Template<'f>> {
        self.templates.get(&guid)
    }

    pub(crate) fn mission_type(&self, guid: Guid) -> Option<&MissionTypeName<'f>> {
        self.types.get(&guid)
    }

    pub(crate) fn reward(&self, guid: Guid) -> Option<i64> {
        self.rewards.get(&guid).copied()
    }

    pub(crate) fn broker(&self, guid: Guid) -> Option<&Broker<'f>> {
        self.brokers.get(&guid)
    }

    /// Returns the localisation key of the display name of the `FactionReputation` Record `guid`.
    pub(crate) fn faction_name(&self, guid: Guid) -> Option<&'f str> {
        self.factions.get(&guid).copied().flatten()
    }

    /// Returns the starmap objects the `MissionLocality` Record `guid` lists as available
    /// locations, in stored order.
    pub(crate) fn locality(&self, guid: Guid) -> &[Guid] {
        self.localities.get(&guid).map_or(&[], Vec::as_slice)
    }

    /// Returns the localisation key of the string variant of organisation `organization` whose
    /// tag is named `variant`, or, without a variant, of its only string variant; an organisation
    /// with several states no choice then.
    pub(crate) fn variant(&self, organization: Guid, variant: Option<&str>) -> Option<&'f str> {
        let variants = self.organizations.get(&organization)?;
        let Some(name) = variant else {
            return match variants.as_slice() {
                [(_, key)] => Some(*key),
                _ => None,
            };
        };
        variants
            .iter()
            .find(|(tag, _)| self.tags.get(*tag).is_some_and(|tag| tag.name() == name))
            .map(|(_, key)| *key)
    }
}

fn piece<'f>(
    trees: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    tags: &TagTree,
) -> Result<Option<Piece<'f>>, p4kx_extraction::DataForgeError> {
    let path = record.record_path()?;
    let starts = |prefix: &str| {
        path.get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
    };
    let wanted = [
        TEMPLATES,
        TYPES,
        ORGANIZATIONS,
        REWARDS,
        LOCALITIES,
        BROKERS,
        FACTIONS,
    ]
    .into_iter()
    .any(starts);
    if !wanted && record.type_name()? != TEMPLATE_TYPE {
        return Ok(None);
    }
    let tree = trees.build(record)?;
    let root = Some(tree.root());
    let guid = tree.guid();
    Ok(Some(if starts(TEMPLATES) || !wanted {
        let mut pools = SpawnPools::default();
        match template_of(&tree, tags, &mut pools) {
            Ok(template) => Piece::Template(guid, template, pools),
            Err(source) => Piece::Failed(CatalogFailure::of(record, &source)),
        }
    } else if starts(TYPES) {
        Piece::Type(
            guid,
            MissionTypeName {
                name_key: key(root, "LocalisedTypeName"),
            },
        )
    } else if starts(ORGANIZATIONS) {
        Piece::Organization(guid, organization_variants(root))
    } else if starts(LOCALITIES) {
        Piece::Locality(guid, available_locations(root))
    } else if starts(BROKERS) {
        Piece::Broker(
            guid,
            Broker {
                title: key(root, "title"),
                description: key(root, "description"),
                mission_type: reference(root, "type").map(|guid| guid.0),
                location: reference(root, "locationMissionAvailable").map(|guid| guid.0),
                objective_orders: objective_orders(root),
            },
        )
    } else if starts(FACTIONS) {
        Piece::Faction(guid, key(root, "displayName"))
    } else {
        match integer(root, "reputationAmount") {
            Some(amount) => Piece::Reward(guid, amount),
            None => return Ok(None),
        }
    }))
}

fn template_of<'f>(
    tree: &RecordTree<'f>,
    tags: &TagTree,
    pools: &mut SpawnPools<'f>,
) -> Result<Template<'f>, SpawnError> {
    let root = Some(tree.root());
    let display = one(root, "contractDisplayInfo");
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
        mission_type: reference(display, "type").map(|guid| guid.0),
        properties: properties(root, "contractProperties"),
        hauling: property_orders(root, "contractProperties"),
        objective_orders: objective_orders(root),
        illegal: boolean(display, "illegal"),
        can_be_shared: items(one(root, "contractClass"), "additionalParams")
            .iter()
            .filter_map(as_struct)
            .find_map(|settings| boolean(Some(settings), "canBeShared")),
        spawns: Some(spawns),
    })
}

fn available_locations(locality: Option<&StructValue<'_>>) -> Vec<Guid> {
    items(locality, "availableLocations")
        .iter()
        .filter_map(|location| match location {
            Value::Reference(target) if target.location().is_none() => Some(target.guid()),
            _ => None,
        })
        .collect()
}

/// Returns the localisation key of the first display string of `display`, a `ContractDisplayInfo`.
pub(crate) fn title_key<'f>(display: Option<&StructValue<'f>>) -> Option<&'f str> {
    match items(display, "displayString").first()? {
        Value::Struct(entry) => key(Some(entry), "value"),
        Value::Locale(text) | Value::String(text) => {
            Some(*text).filter(|text| text.starts_with('@') && !is_placeholder_key(text))
        }
        _ => None,
    }
}

fn organization_variants<'f>(organization: Option<&StructValue<'f>>) -> Variants<'f> {
    let container = one(organization, "stringVariants");
    items(container, "variants")
        .iter()
        .filter_map(as_struct)
        .filter_map(|variant| {
            let tag = reference(Some(variant), "tag")?;
            Some((tag.0, key(Some(variant), "string")?))
        })
        .collect()
}

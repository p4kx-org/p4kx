// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{CancelToken, DataForge, Guid, Record, TagTree, TreeBuilder};
use rayon::prelude::*;

use crate::contract_hauling::HaulingRequirement;
use crate::export_ledger::ExportLedger;
use crate::mission_hauling::HaulingProperty;
use crate::mission_layers::Property;
use crate::spawn_layers::Level;
use crate::spawn_pools::SpawnPools;
use crate::{CatalogError, CatalogFailure};

#[path = "mission_index_read/mod.rs"]
mod read;

pub(crate) use read::title_key;

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
    ledger: ExportLedger,
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
        let (tags, failures, ledger) = read::read_tags(forge, cancel)?;
        let pieces = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                read::read(trees, record, &tags, cancel)
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, _>>()?;
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
            failures,
            ledger,
        };
        for read in pieces {
            index.ledger.merge(read.ledger);
            if let Some(piece) = read.piece {
                index.insert(piece);
            }
        }
        Ok(index)
    }

    fn insert(&mut self, piece: Piece<'f>) {
        match piece {
            Piece::Template(guid, template, found) => {
                self.templates.insert(guid, template);
                self.pools.merge(found);
            }
            Piece::Failed(failure) => self.failures.push(failure),
            Piece::Type(guid, kind) => {
                self.types.insert(guid, kind);
            }
            Piece::Organization(guid, variants) => {
                self.organizations.insert(guid, variants);
            }
            Piece::Reward(guid, amount) => {
                self.rewards.insert(guid, amount);
            }
            Piece::Locality(guid, locations) => {
                self.localities.insert(guid, locations);
            }
            Piece::Broker(guid, broker) => {
                self.brokers.insert(guid, broker);
            }
            Piece::Faction(guid, name) => {
                self.factions.insert(guid, name);
            }
        }
    }

    /// Takes the spawn descriptions and location queries of the templates and the templates that
    /// could not be read.
    pub(crate) fn take_template_results(
        &mut self,
    ) -> (SpawnPools<'f>, Vec<CatalogFailure>, ExportLedger) {
        (
            std::mem::take(&mut self.pools),
            std::mem::take(&mut self.failures),
            std::mem::take(&mut self.ledger),
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

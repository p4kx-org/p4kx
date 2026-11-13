// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Guid, Record, SourceEvidenceError, TagTree, TreeBuilder,
};
use rayon::prelude::*;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::RewardRead;
use crate::contract_hauling::HaulingRequirement;
use crate::export_ledger::ExportLedger;
use crate::mission_hauling::HaulingProperty;
use crate::mission_text_inputs::{BrokerTexts, MissionItemTexts, OrganizationTexts, TemplateTexts};
use crate::spawn_layers::Level;
use crate::spawn_pools::SpawnPools;
use crate::{CatalogError, CatalogReadFailure};

#[path = "mission_index_read/nominal_reward.rs"]
mod nominal_reward;
#[path = "mission_index_read/mod.rs"]
mod read;
#[path = "mission_index_read/template_facts.rs"]
mod template_facts;
#[path = "mission_index_read/template_read.rs"]
mod template_read;

pub(crate) use self::template_facts::TemplateFacts;
use self::template_read::{FailedTemplate, TemplateReadSource};

/// A `MissionBrokerEntry` Record: what a contract without a template states about itself.
#[derive(Debug, Default)]
pub(crate) struct Broker<'f> {
    pub(crate) texts: Option<BrokerTexts<'f>>,
    pub(crate) mission_type: Option<Guid>,
    pub(crate) location: Option<Guid>,
    pub(crate) objective_orders: Vec<HaulingRequirement>,
}

/// What a `ContractTemplate` Record contributes to a contract: the title, the mission type and the
/// `illegal` flag of its display info, its properties and the `canBeShared` flag of its contract
/// class.
#[derive(Debug, Default)]
pub(crate) struct Template<'f> {
    pub(crate) texts: Option<TemplateTexts<'f>>,
    pub(crate) mission_type: Option<Guid>,
    pub(crate) hauling: Vec<HaulingProperty<'f>>,
    pub(crate) objective_orders: Vec<HaulingRequirement>,
    pub(crate) facts: Option<Box<TemplateFacts>>,
    /// The ship spawns and location queries of the template.
    pub(crate) spawns: Option<Level>,
}

/// A `MissionType` Record: the key of its localised name.
#[derive(Debug)]
pub(crate) struct MissionTypeName<'f> {
    pub(crate) name_key: Option<&'f str>,
}

enum RewardDependency {
    Original(RewardRead),
    UnreadablePath(RewardRead),
    #[cfg(test)]
    Accounting(Option<i64>),
}

#[cfg(test)]
impl RewardDependency {
    const fn amount(&self) -> Option<i64> {
        match self {
            Self::Original(reward) => reward.amount(),
            Self::UnreadablePath(_) => None,
            #[cfg(test)]
            Self::Accounting(value) => *value,
        }
    }
}

enum Piece<'f> {
    Template(Guid, Template<'f>, SpawnPools<'f>),
    Failed(CatalogReadFailure<'f>),
    FailedTemplate(CatalogReadFailure<'f>, FailedTemplate),
    Type(Guid, MissionTypeName<'f>),
    Organization(Guid, Option<OrganizationTexts<'f>>),
    Item((Guid, u32), MissionItemTexts<'f>),
    Reward(Guid, u32, RewardDependency),
    UnusableRewardCandidate(CatalogReadFailure<'f>, RewardDependency),
    Locality(Guid, Vec<Guid>),
    Broker(Guid, Broker<'f>),
    Faction(Guid, Option<&'f str>),
}

/// The Records a contract refers to, read once: templates, mission types, mission
/// organisations, reputation rewards, mission localities, broker entries, faction reputations
/// and the tag tree.
#[derive(Default)]
pub(crate) struct MissionIndex<'f> {
    templates: HashMap<Guid, Template<'f>>,
    failed_templates: HashMap<Guid, Vec<FailedTemplate>>,
    template_source: Option<TemplateReadSource>,
    types: HashMap<Guid, MissionTypeName<'f>>,
    organizations: HashMap<Guid, OrganizationTexts<'f>>,
    items: HashMap<(Guid, u32), MissionItemTexts<'f>>,
    rewards: BTreeMap<(Guid, u32), RewardDependency>,
    localities: HashMap<Guid, Vec<Guid>>,
    brokers: HashMap<Guid, Broker<'f>>,
    factions: HashMap<Guid, Option<&'f str>>,
    tags: Tags,
    pools: SpawnPools<'f>,
    failures: Vec<CatalogReadFailure<'f>>,
    ledger: ExportLedger,
}

enum Tags {
    Owned(TagTree),
    Captured(crate::catalog_v3::tag_catalog::CapturedTagTree),
}

impl Default for Tags {
    fn default() -> Self {
        Self::Owned(TagTree::default())
    }
}

impl Tags {
    fn tree(&self) -> &TagTree {
        match self {
            Self::Owned(tree) => tree,
            Self::Captured(tree) => tree.tree(),
        }
    }
}

impl<'f> MissionIndex<'f> {
    /// Reads the complete dependency index only when selected catalog mission inputs need it.
    #[cfg(test)]
    pub(crate) fn read_catalog(
        forge: &'f DataForge<'f>,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        match filter {
            Some(filter) if !read::catalog_needs_index(forge, filter, cancel)? => {
                Ok(Self::default())
            }
            Some(_) | None => Self::read(forge, cancel),
        }
    }

    pub(crate) fn read_captured(
        forge: &'f DataForge<'f>,
        input: &OriginalCatalogInput<'_, 'f>,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        dependencies: Option<crate::catalog_v3::tag_catalog::TagDependencies<'f>>,
        captured: &crate::catalog_v3::tag_catalog::CapturedTags<'_, 'f>,
    ) -> Result<Self, CatalogError> {
        input.check_forge(forge)?;
        if !Self::needs_index(forge, filter, cancel)? {
            if dependencies.is_some() {
                return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
            }
            return Ok(Self::default());
        }
        let dependencies =
            dependencies.ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
        if !std::ptr::eq(dependencies.tree.tree(), captured.tree()) {
            return Err(crate::ExportAccountingError::ConflictingTagTrace.into());
        }
        let tags = Tags::Captured(dependencies.tree);
        Self::from_tags(
            forge,
            Some(input),
            cancel,
            tags,
            dependencies.failures,
            dependencies.ledger,
            Some(captured),
        )
    }
    pub(crate) fn needs_index(
        forge: &DataForge<'_>,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
    ) -> Result<bool, CatalogError> {
        filter.map_or(Ok(true), |filter| {
            read::catalog_needs_index(forge, filter, cancel)
        })
    }

    /// Reads the Records of `forge` a contract refers to; a Record that cannot be read is left
    /// out, so what refers to it resolves to nothing.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
    #[cfg(test)]
    pub(crate) fn read(
        forge: &'f DataForge<'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        Self::read_observed(forge, None, cancel)
    }

    #[cfg(test)]
    fn read_observed(
        forge: &'f DataForge<'f>,
        input: Option<&OriginalCatalogInput<'_, 'f>>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let (tags, failures, ledger) = read::read_tags(forge, input, cancel)?;
        Self::from_tags(
            forge,
            input,
            cancel,
            Tags::Owned(tags),
            failures,
            ledger,
            None,
        )
    }

    fn from_tags(
        forge: &'f DataForge<'f>,
        input: Option<&OriginalCatalogInput<'_, 'f>>,
        cancel: Option<&CancelToken>,
        tags: Tags,
        failures: Vec<CatalogReadFailure<'f>>,
        ledger: ExportLedger,
        captured: Option<&crate::catalog_v3::tag_catalog::CapturedTags<'_, 'f>>,
    ) -> Result<Self, CatalogError> {
        let records: Vec<Record<'f>> = forge.records().collect();
        let pieces = records
            .par_iter()
            .map_init(TreeBuilder::new, |trees, record| {
                read::read(trees, record, tags.tree(), input, captured, cancel)
            })
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, _>>()?;
        let mut index = Self {
            tags,
            failures,
            ledger,
            template_source: input.map(|input| TemplateReadSource::new(input.origins())),
            ..Self::default()
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
                self.failed_templates.remove(&guid);
                self.templates.insert(guid, template);
                self.pools.merge(found);
            }
            Piece::Failed(failure) => self.failures.push(failure),
            Piece::FailedTemplate(failure, attempt) => {
                let guid = failure.guid();
                if !self.templates.contains_key(&guid) {
                    self.failed_templates.entry(guid).or_default().push(attempt);
                }
                self.failures.push(failure);
            }
            Piece::Type(guid, kind) => {
                self.types.insert(guid, kind);
            }
            Piece::Organization(guid, variants) => {
                if let Some(variants) = variants {
                    self.organizations.insert(guid, variants);
                }
            }
            Piece::Item(key, variants) => {
                self.items.insert(key, variants);
            }
            Piece::Reward(guid, position, amount) => {
                self.rewards.insert((guid, position), amount);
            }
            Piece::UnusableRewardCandidate(failure, reward) => {
                self.rewards
                    .insert((failure.guid(), failure.record().index()), reward);
                self.failures.push(failure);
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
    ) -> (SpawnPools<'f>, Vec<CatalogReadFailure<'f>>, ExportLedger) {
        (
            std::mem::take(&mut self.pools),
            std::mem::take(&mut self.failures),
            std::mem::take(&mut self.ledger),
        )
    }

    /// Returns the tag tree of the DataForge.
    pub(crate) fn tags(&self) -> &TagTree {
        self.tags.tree()
    }

    pub(crate) fn template(&self, guid: Guid) -> Option<&Template<'f>> {
        self.templates.get(&guid)
    }

    pub(crate) fn failed_templates(&self, guid: Guid) -> &[FailedTemplate] {
        self.failed_templates.get(&guid).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn check_template_source(
        &self,
        origins: &CatalogOrigins,
    ) -> Result<(), CatalogError> {
        self.template_source
            .as_ref()
            .ok_or(SourceEvidenceError::ForeignEvidence)?
            .check(origins)
    }

    pub(crate) fn mission_type(&self, guid: Guid) -> Option<&MissionTypeName<'f>> {
        self.types.get(&guid)
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

    /// Returns the actual ordered authored text variants of the chosen dependency.
    pub(crate) fn item(&self, guid: Guid, record_index: u32) -> Option<&MissionItemTexts<'f>> {
        self.items.get(&(guid, record_index))
    }

    pub(crate) fn organization(&self, guid: Guid) -> Option<&OrganizationTexts<'f>> {
        self.organizations.get(&guid)
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use p4kx_dataforge::{DataForge, Guid, Record, RecordTree, TagTree, TreeBuilder};

use crate::assortment::Assorted;
use crate::entities::is_entity;
use crate::pick::LoadoutPicks;
use crate::plan::Plan;
use crate::spec::{EntitySpec, EntrySpec, LoadoutSpec, TargetSpec};
use crate::walk::Walk;
use crate::{
    ChangeError, ChangedLoadout, Entities, EntityNode, ItemFacts, LandingGear, LoadoutChanges,
    LoadoutError, LoadoutFiles, VehicleImplementation, Wearable, xml,
};

pub(crate) type SpecResult = Arc<Result<EntitySpec, LoadoutError>>;
pub(crate) type EntriesResult = Result<Entries, LoadoutError>;
pub(crate) type VehicleResult = Result<Arc<VehicleImplementation>, LoadoutError>;

/// The entries of a Loadout and why parts of it cannot be read.
#[derive(Debug, Clone)]
pub(crate) struct Entries {
    pub(crate) list: Arc<[EntrySpec]>,
    pub(crate) issues: Arc<[LoadoutError]>,
}

impl Entries {
    fn complete(list: Arc<[EntrySpec]>) -> Self {
        Self {
            list,
            issues: Arc::from([]),
        }
    }
}

/// Resolves the default Loadouts of Entities into [`EntityNode`] trees.
///
/// Records are read once per resolver and kept, so resolving several Entities with one resolver
/// reads each Record once; the Records an Entity reaches are read in parallel. Resolution is
/// cycle-safe and bounded by [`MAX_LOADOUT_DEPTH`](crate::MAX_LOADOUT_DEPTH) and
/// [`MAX_LOADOUT_PORTS`](crate::MAX_LOADOUT_PORTS); a port that cannot be resolved is kept in the
/// tree with the reason.
pub struct LoadoutResolver<'f> {
    pub(crate) entities: Arc<Entities<'f>>,
    pub(crate) files: Option<&'f dyn LoadoutFiles>,
    pub(crate) specs: HashMap<Guid, SpecResult>,
    xml: HashMap<String, EntriesResult>,
    pub(crate) vehicles: HashMap<String, VehicleResult>,
    pub(crate) landing: HashMap<Guid, Result<Arc<[LandingGear]>, LoadoutError>>,
    pub(crate) builder: TreeBuilder<'f>,
    pub(crate) assortments: HashMap<Guid, Assorted>,
    pub(crate) assorting: Vec<Guid>,
    pub(crate) picks: LoadoutPicks,
    /// Lower-case names of the assortment groups met while resolving with picks.
    pub(crate) seen_groups: HashSet<String>,
    pub(crate) stop_at_docked: bool,
    stop: Option<&'f (dyn Fn() -> bool + Sync)>,
    serial_reads: usize,
    pub(crate) tag_tree: OnceLock<TagTree>,
}

impl std::fmt::Debug for LoadoutResolver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadoutResolver")
            .field("entities", &self.entities)
            .field("files", &self.files.is_some())
            .field("records", &self.specs.len())
            .field("stop_at_docked", &self.stop_at_docked)
            .finish_non_exhaustive()
    }
}

impl<'f> LoadoutResolver<'f> {
    /// Creates a resolver over `forge` that reports XML Loadouts as not read.
    #[must_use]
    pub fn new(forge: &'f DataForge<'f>) -> Self {
        Self::with_entities(Arc::new(Entities::new(forge)))
    }

    /// Creates a resolver over the DataForge of `entities`, sharing the index with every other
    /// resolver built from the same `Arc`, so that the Records are walked once.
    #[must_use]
    pub fn with_entities(entities: Arc<Entities<'f>>) -> Self {
        Self {
            entities,
            files: None,
            specs: HashMap::new(),
            xml: HashMap::new(),
            vehicles: HashMap::new(),
            landing: HashMap::new(),
            builder: TreeBuilder::new(),
            assortments: HashMap::new(),
            assorting: Vec::new(),
            picks: LoadoutPicks::new(),
            seen_groups: HashSet::new(),
            stop_at_docked: false,
            stop: None,
            serial_reads: 0,
            tag_tree: OnceLock::new(),
        }
    }

    /// Stops at docked vehicles: an Entity whose `AttachDef.Type` starts with `NOITEM_Vehicle`
    /// stays attached to its port and is marked, but its Loadout and ports are not built unless
    /// it is the Entity being resolved, which is never marked. By default docked vehicles are expanded like any Entity.
    #[must_use]
    pub const fn stopping_at_docked_vehicles(mut self) -> Self {
        self.stop_at_docked = true;
        self
    }

    /// Polls `stop` while resolving with [`resolve_with`](Self::resolve_with), between the ports
    /// it builds and the levels it reads ahead; once it returns `true` the resolution ends with
    /// [`ChangeError::Cancelled`]. Only `resolve_with` ends that way; `resolve` ignores it.
    #[must_use]
    pub fn with_stop(mut self, stop: &'f (dyn Fn() -> bool + Sync)) -> Self {
        self.stop = Some(stop);
        self
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stop.is_some_and(|stop| stop())
    }

    /// Reads the XML Loadouts Records name from `files`.
    #[must_use]
    pub fn with_files(mut self, files: &'f dyn LoadoutFiles) -> Self {
        self.files = Some(files);
        self.xml.clear();
        self.vehicles.clear();
        self
    }

    /// Returns the Entities, to find the one to resolve.
    #[must_use]
    pub fn entities(&self) -> &Entities<'f> {
        &self.entities
    }

    /// Resolves the default Loadout of `entity` into its tree.
    ///
    /// # Errors
    ///
    /// Returns [`LoadoutError::NotAnEntity`] when the Record is no Entity and
    /// [`LoadoutError::UnreadableRecord`] when its tree cannot be built; every error below the
    /// root is kept in the tree instead.
    pub fn resolve(&mut self, entity: &Record<'f>) -> Result<EntityNode, LoadoutError> {
        self.picks = LoadoutPicks::new();
        self.prefetch(*entity);
        let spec = self.spec(entity.guid());
        let spec = spec.as_ref().as_ref().map_err(Clone::clone)?;
        let mut walk = Walk::new(entity.guid(), None);
        let mut node = self.node(spec, None, &mut walk);
        self.dress_body(&mut node);
        Ok(node)
    }

    /// Resolves the default Loadout of `entity` like [`resolve`](Self::resolve), taking the
    /// Entity's own Record from `tree`, which the caller already built from it, so that the
    /// Record is not built again. A `tree` of another Record is ignored.
    ///
    /// # Errors
    ///
    /// As [`resolve`](Self::resolve).
    pub fn resolve_tree(
        &mut self,
        entity: &Record<'f>,
        tree: &RecordTree<'f>,
    ) -> Result<EntityNode, LoadoutError> {
        if tree.guid() == entity.guid()
            && !self.specs.contains_key(&entity.guid())
            && is_entity(entity)
        {
            self.specs
                .insert(entity.guid(), Arc::new(Ok(EntitySpec::read(tree))));
        }
        self.resolve(entity)
    }

    /// Resolves the default Loadout of `entity` with `changes` applied: the ports they exclude
    /// hold nothing and have the source [`PortSource::Excluded`](crate::PortSource::Excluded);
    /// the ports they replace hold the replacing Entity with its own Loadout and have the source
    /// [`PortSource::Replaced`](crate::PortSource::Replaced). An Entity that does not fit the
    /// port's [`PortNode::constraints`](crate::PortNode::constraints) is attached all the same;
    /// [`resolve_changed`](Self::resolve_changed) reports the mismatch.
    ///
    /// # Errors
    ///
    /// Returns the first of: [`ChangeError::Loadout`] when `entity` cannot be resolved, a change
    /// that is no port path or contradicts another, a replacement whose Entity cannot be found
    /// or read, and a change whose port the Loadout does not have, and
    /// [`ChangeError::Cancelled`] once the stop of [`with_stop`](Self::with_stop) returned
    /// `true`.
    pub fn resolve_with(
        &mut self,
        entity: &Record<'f>,
        changes: &LoadoutChanges,
    ) -> Result<EntityNode, ChangeError> {
        self.resolve_changed(entity, changes)
            .map(ChangedLoadout::into_node)
    }

    /// Resolves like [`resolve_with`](Self::resolve_with) and also returns the changes that lie
    /// below a docked vehicle the resolver does not expand
    /// ([`stopping_at_docked_vehicles`](Self::stopping_at_docked_vehicles)): they are not
    /// applied and not an error. It also returns the replacements that do not fit their port.
    ///
    /// # Errors
    ///
    /// As [`resolve_with`](Self::resolve_with).
    pub fn resolve_changed(
        &mut self,
        entity: &Record<'f>,
        changes: &LoadoutChanges,
    ) -> Result<ChangedLoadout, ChangeError> {
        let outcome = self.resolve_walk(entity, changes);
        if self.stopped() {
            return Err(ChangeError::Cancelled);
        }
        outcome
    }

    fn resolve_walk(
        &mut self,
        entity: &Record<'f>,
        changes: &LoadoutChanges,
    ) -> Result<ChangedLoadout, ChangeError> {
        self.picks = changes.picks().clone();
        self.seen_groups.clear();
        let plan = if changes.has_no_port_changes() {
            None
        } else {
            Some(Plan::new(changes, |path, text| {
                self.replacement(path, text)
            })?)
        };
        self.prefetch(*entity);
        let spec = self.spec(entity.guid());
        let spec = spec.as_ref().as_ref().map_err(Clone::clone)?;
        let mut walk = Walk::new(entity.guid(), plan);
        walk.cancellable = true;
        let mut node = self.node(spec, None, &mut walk);
        self.dress_body(&mut node);
        node.issues.extend(self.unknown_picks());
        let (ignored, mismatches) = match walk.plan.take() {
            Some(plan) => plan.finish()?,
            None => (Vec::new(), Vec::new()),
        };
        Ok(ChangedLoadout {
            node,
            ignored,
            mismatches,
        })
    }

    /// Returns what `entity` offers a port, for [`PortNode::accepts`](crate::PortNode::accepts).
    ///
    /// # Errors
    ///
    /// Returns the reason when the Record is no Entity or cannot be read.
    pub fn item_facts(&mut self, entity: &Record<'f>) -> Result<ItemFacts, LoadoutError> {
        let spec = self.spec(entity.guid());
        spec.as_ref()
            .as_ref()
            .map(EntitySpec::item_facts)
            .map_err(Clone::clone)
    }

    /// Reads what `entity` offers to be worn on a body: its geometry and its sub-geometry per body
    /// tag. Its Loadout is not resolved.
    ///
    /// # Errors
    ///
    /// Returns the reason when the Record is no Entity or cannot be read.
    pub fn wearable(&mut self, entity: &Record<'f>) -> Result<Wearable, LoadoutError> {
        let spec = self.spec(entity.guid());
        spec.as_ref()
            .as_ref()
            .map(Wearable::from_spec)
            .map_err(Clone::clone)
    }

    /// Finds and reads the Entity `text` names for the replacement at `path`, and reads the
    /// Records its Loadout reaches.
    fn replacement(&mut self, path: &str, text: &str) -> Result<(Guid, ItemFacts), ChangeError> {
        let record = self
            .entities
            .find(text)
            .map_err(|source| ChangeError::Entity {
                path: path.to_owned(),
                source,
            })?;
        self.prefetch(record);
        let facts = self
            .item_facts(&record)
            .map_err(|source| ChangeError::Unreadable {
                path: path.to_owned(),
                entity: text.to_owned(),
                source,
            })?;
        Ok((record.guid(), facts))
    }

    /// Returns the cached spec of the Record `guid`, reading it on a miss.
    pub(crate) fn spec(&mut self, guid: Guid) -> SpecResult {
        if let Some(spec) = self.specs.get(&guid) {
            return Arc::clone(spec);
        }
        self.serial_reads = self.serial_reads.saturating_add(1);
        let forge = self.entities.forge();
        let spec = Arc::new(match forge.record_by_guid(guid) {
            Some(record) => read_spec(&mut self.builder, &record),
            None => Err(LoadoutError::DanglingReference { guid }),
        });
        self.specs.insert(guid, Arc::clone(&spec));
        spec
    }

    /// Returns the entries of a Loadout, reading an XML Loadout once per resolver.
    pub(crate) fn entries(&mut self, loadout: Option<&LoadoutSpec>) -> EntriesResult {
        match loadout {
            None => Ok(Entries::complete(Arc::from([]))),
            Some(LoadoutSpec::Manual(entries)) => Ok(Entries::complete(Arc::clone(entries))),
            Some(LoadoutSpec::Layered(layers)) => self.layered_entries(layers),
            Some(LoadoutSpec::Assortments(guids)) => Ok(self.assortment_entries(guids)),
            Some(LoadoutSpec::Xml(path)) => {
                let Some(files) = self.files else {
                    return Err(LoadoutError::NoLoadoutFiles {
                        path: path.to_string(),
                    });
                };
                self.xml
                    .entry(path.to_ascii_lowercase())
                    .or_insert_with(|| {
                        xml::load(files, path).map(|loaded| Entries {
                            list: Arc::from(loaded.entries),
                            issues: Arc::from(loaded.issues),
                        })
                    })
                    .clone()
            }
        }
    }

    /// Returns the Record a target names, or `None` when there is none.
    pub(crate) fn record_of(&self, target: &TargetSpec) -> Option<Record<'f>> {
        match target {
            TargetSpec::Reference(guid) => self.entities.forge().record_by_guid(*guid),
            TargetSpec::ClassName(name) => self.entities.by_class_name(name),
            TargetSpec::None | TargetSpec::Invalid(_) => None,
        }
    }

    /// Returns how many Records have been read into specs so far.
    #[doc(hidden)]
    #[must_use]
    pub fn records_read(&self) -> usize {
        self.specs.len()
    }

    /// Returns how many Records were read one at a time while a tree was built, outside the
    /// parallel prefetch.
    #[doc(hidden)]
    #[must_use]
    pub const fn serial_reads(&self) -> usize {
        self.serial_reads
    }
}
/// Reads the spec of one Record, which must be an Entity.
pub(crate) fn read_spec<'f>(
    builder: &mut TreeBuilder<'f>,
    record: &Record<'f>,
) -> Result<EntitySpec, LoadoutError> {
    let guid = record.guid();
    if !is_entity(record) {
        return Err(LoadoutError::NotAnEntity {
            guid,
            type_name: record
                .type_name()
                .unwrap_or("Record of an unreadable type")
                .to_owned(),
        });
    }
    builder
        .build(record)
        .map(|tree| EntitySpec::read(&tree))
        .map_err(|error| LoadoutError::UnreadableRecord {
            guid,
            reason: error.to_string(),
        })
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{RecordTree, StructValue, TagTree};

use crate::catalog_read::original::RecordSubject;
use crate::guid_text::CatalogGuid;
use crate::mission_property_set::{MissionPropertyLevel, MissionPropertySet, MissionPropertySets};
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::spawn_pools::SpawnPools;
use crate::spawn_read::Reader;
use crate::template_property_read::PropertyReader;

#[path = "mission_property_set_links.rs"]
pub(crate) mod links;

use links::MissionPropertySetLinks;
pub(crate) use links::identity;

/// Canonical property-set registration for one immutable owning Record.
pub(crate) struct MissionPropertySetRegistry<'r, 'f> {
    tree: &'r RecordTree<'f>,
    subject: RecordSubject,
    links: MissionPropertySetLinks<'r, 'f>,
    reader: Reader<'r, 'f>,
    capture: &'r dyn FieldCapture<'f>,
    spawns: RefCell<SpawnPools<'f>>,
    sets: RefCell<MissionPropertySets<'f>>,
}

impl<'r, 'f> MissionPropertySetRegistry<'r, 'f> {
    pub(crate) fn new_observed(
        admission: &RecordObservation<'_, '_>,
        tree: &'r RecordTree<'f>,
        tags: &'r TagTree,
        capture: &'r dyn FieldCapture<'f>,
    ) -> Result<Self, ReadError> {
        admission.check_tree(tree)?;
        capture.check_parent(tree.root())?;
        Self::create(tree, admission.subject().clone(), tags, capture)
    }

    fn create(
        tree: &'r RecordTree<'f>,
        subject: RecordSubject,
        tags: &'r TagTree,
        capture: &'r dyn FieldCapture<'f>,
    ) -> Result<Self, ReadError> {
        let registry = Self {
            tree,
            subject,
            links: MissionPropertySetLinks::new(tree)?,
            reader: Reader::for_observed(tags, tree, capture)?,
            capture,
            spawns: RefCell::new(SpawnPools::default()),
            sets: RefCell::new(MissionPropertySets::default()),
        };
        crate::mission_property_sources::visit(tree.root(), capture, |level, params| {
            registry.register(level, params).map(|_| ())
        })?;
        Ok(registry)
    }

    /// Returns authored handler, contract and sub-contract set links in that order.
    pub(crate) fn links(
        &self,
        handler: &'r StructValue<'f>,
        contract: &'r StructValue<'f>,
        sub: Option<&'r StructValue<'f>>,
    ) -> Result<Vec<CatalogGuid>, ReadError> {
        links::row_scopes(handler, contract, sub, self.capture)?
            .into_iter()
            .filter_map(|scope| self.register(scope.level, scope.parameters).transpose())
            .collect()
    }

    fn register(
        &self,
        level: MissionPropertyLevel,
        params: Option<&'r StructValue<'f>>,
    ) -> Result<Option<CatalogGuid>, ReadError> {
        let Some(source) = self.links.source(params, self.capture)? else {
            return Ok(None);
        };
        let guid = source.guid;
        if self.sets.borrow().contains(guid) {
            return Ok(Some(guid));
        }
        let reader = PropertyReader::new(&self.reader, &self.spawns);
        let properties = reader.source_properties(source.parameters, source.values)?;
        let body = self
            .capture
            .record_field(source.parameters, source.value, &source.location)?;
        self.sets.borrow_mut().insert_subject(
            self.subject.clone(),
            MissionPropertySet {
                guid,
                record_guid: CatalogGuid(self.tree.guid()),
                record_path: self.tree.record_path(),
                source_type: self.tree.type_name(),
                source_location: source.location,
                level,
                properties,
            },
            body,
        )?;
        Ok(Some(guid))
    }

    /// Returns canonical rows and any spawn definitions referenced by their typed values.
    pub(crate) fn finish(self) -> (MissionPropertySets<'f>, SpawnPools<'f>) {
        (self.sets.into_inner(), self.spawns.into_inner())
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{RecordTree, StructValue, TagTree, Value};

use crate::guid_text::CatalogGuid;
use crate::mission_property_fields as fields;
use crate::mission_property_set::{MissionPropertyLevel, MissionPropertySet, MissionPropertySets};
use crate::mission_property_set_index::MissionPropertySetIndex;
use crate::record_read::ReadError;
use crate::spawn_model::content_guid;
use crate::spawn_pools::SpawnPools;
use crate::spawn_read::Reader;
use crate::template_issues::Unknowns;
use crate::template_property_read::PropertyReader;
use crate::texts::Texts;

/// Canonical property-set registration for one immutable owning Record.
pub(crate) struct MissionPropertySetRegistry<'r, 'f> {
    tree: &'r RecordTree<'f>,
    index: MissionPropertySetIndex<'r, 'f>,
    reader: Reader<'r, 'f>,
    texts: &'r Texts<'f>,
    unknowns: &'r Unknowns,
    spawns: RefCell<SpawnPools<'f>>,
    sets: RefCell<MissionPropertySets<'f>>,
}

impl<'r, 'f> MissionPropertySetRegistry<'r, 'f> {
    /// Creates the source registry without copying authored nodes or values.
    pub(crate) fn new(
        tree: &'r RecordTree<'f>,
        tags: &'r TagTree,
        texts: &'r Texts<'f>,
        unknowns: &'r Unknowns,
    ) -> Result<Self, ReadError> {
        let registry = Self {
            tree,
            index: MissionPropertySetIndex::new(tree.root())?,
            reader: Reader::for_record(tags, tree),
            texts,
            unknowns,
            spawns: RefCell::new(SpawnPools::default()),
            sets: RefCell::new(MissionPropertySets::default()),
        };
        crate::mission_property_sources::visit(tree.root(), |level, params| {
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
        let handler = fields::parameter(handler, "contractParams")?;
        let contract = fields::parameter(contract, "paramOverrides")?;
        let sub = sub
            .map(|sub| fields::parameter(sub, "paramOverrides").map(|params| params.or(Some(sub))))
            .transpose()?
            .flatten();
        [
            (MissionPropertyLevel::Handler, handler),
            (MissionPropertyLevel::Contract, contract),
            (MissionPropertyLevel::SubContract, sub),
        ]
        .into_iter()
        .filter_map(|(level, params)| self.register(level, params).transpose())
        .collect()
    }

    fn register(
        &self,
        level: MissionPropertyLevel,
        params: Option<&'r StructValue<'f>>,
    ) -> Result<Option<CatalogGuid>, ReadError> {
        let Some(params) = params else {
            return Ok(None);
        };
        let Some(value) = params.field("propertyOverrides") else {
            return Ok(None);
        };
        if matches!(value, Value::Null) {
            return Ok(None);
        }
        let location = self
            .index
            .location(params)
            .ok_or("property source is outside its original Record")?;
        let source_location = if location.is_empty() {
            "propertyOverrides".to_owned()
        } else {
            format!("{location}.propertyOverrides")
        };
        let guid = content_guid(&(
            "missionPropertySet",
            CatalogGuid(self.tree.guid()),
            &source_location,
        ))?;
        if self.sets.borrow().contains(guid) {
            return Ok(Some(guid));
        }
        let Value::Array(values) = value else {
            return Err(fields::malformed(
                "propertyOverrides",
                value,
                "an authored property array",
            ));
        };
        let reader = PropertyReader::new(&self.reader, self.texts, self.unknowns, &self.spawns);
        let properties = reader.source_properties(values)?;
        self.sets.borrow_mut().insert(MissionPropertySet {
            guid,
            record_guid: CatalogGuid(self.tree.guid()),
            record_path: self.tree.record_path(),
            source_type: self.tree.type_name(),
            source_location,
            level,
            properties,
        });
        Ok(Some(guid))
    }

    /// Returns canonical rows and any spawn definitions referenced by their typed values.
    pub(crate) fn finish(self) -> (MissionPropertySets<'f>, SpawnPools<'f>) {
        (self.sets.into_inner(), self.spawns.into_inner())
    }
}

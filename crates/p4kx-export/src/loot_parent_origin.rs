// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, SourceUses, SourcedValue};

use crate::CatalogError;
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::{NoObject, Objects};
use crate::starmap_walk::Opened;

pub(super) struct ParentOrigins<'source, 'object> {
    objects: &'source Objects<'object>,
    sources: &'source SourceUses,
    by_start: HashMap<Option<Guid>, SourcedValue<CatalogGuid>>,
}

impl<'source, 'object> ParentOrigins<'source, 'object> {
    pub(super) fn new(objects: &'source Objects<'object>, sources: &'source SourceUses) -> Self {
        Self {
            objects,
            sources,
            by_start: HashMap::new(),
        }
    }

    pub(super) fn project(
        &mut self,
        opened: &Opened,
        location: Result<CatalogGuid, NoObject>,
        system: &SourcedValue<String>,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let selected = match self.by_start.entry(opened.under) {
            std::collections::hash_map::Entry::Occupied(known) => known.into_mut(),
            std::collections::hash_map::Entry::Vacant(vacant) => vacant.insert(
                self.objects
                    .location_origin(opened.under, location, self.sources)?,
            ),
        };
        let (inputs, known) = opened.parent.inputs()?;
        let ancestry = self.sources.origin(&inputs, known);
        Ok(self
            .sources
            .add_value_origins(selected.clone(), &[system.origin(), &ancestry])?)
    }
}

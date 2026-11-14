// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutTargetGuid;
use std::sync::Arc;

use p4kx_dataforge::{Guid, Record};

use crate::resolver::SpecResult;
use crate::resolver_observation::{self, LoadoutReadEvent};
use crate::{EntityNode, LoadoutError, LoadoutResolver};

impl<'f> LoadoutResolver<'f> {
    pub(crate) fn spec(&mut self, guid: Guid) -> SpecResult {
        self.entities.forge().record_by_guid(guid).map_or_else(
            || {
                Arc::new(Err(LoadoutError::DanglingReference {
                    guid: LoadoutTargetGuid::unbound(guid),
                }))
            },
            |record| self.spec_record(&record),
        )
    }

    pub(crate) fn spec_record(&mut self, record: &Record<'f>) -> SpecResult {
        if self.observation_stopped() {
            return Arc::new(Err(LoadoutError::ObservationStopped));
        }
        if let Some(spec) = self.specs.get(&record.index()) {
            if let Err(error) = resolver_observation::notify(
                self.observation.as_deref(),
                LoadoutReadEvent::Reused { record },
            ) {
                return Arc::new(Err(error));
            }
            return Arc::clone(spec);
        }
        self.serial_reads = self.serial_reads.saturating_add(1);
        let spec = Arc::new(resolver_observation::read_spec(
            &mut self.builder,
            record,
            self.observation.as_deref(),
        ));
        self.specs.insert(record.index(), Arc::clone(&spec));
        spec
    }

    pub(crate) fn spec_of(&mut self, node: &EntityNode) -> Option<SpecResult> {
        let record_index = node.record_index?;
        Some(self.entities.forge().record(record_index).filter(|record| record.guid() == node.guid)
            .map_or_else(
                || Arc::new(Err(LoadoutError::UnreadableRecord {
                    guid: LoadoutTargetGuid::unbound(node.guid),
                    reason: format!("the physical source Record {record_index} is absent from this DataForge"),
                })),
                |record| self.spec_record(&record),
            ))
    }
}

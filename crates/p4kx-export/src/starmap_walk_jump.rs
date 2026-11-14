// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceValue, SourcedValue};

use super::{System, WalkPath};
use crate::catalog_read::original::PhysicalEntryRef;
use crate::entry_source::EntrySource;
use crate::guid_text::CatalogGuid;
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::jump_endpoint::{GateResolution, JumpOccurrence};
use crate::starmap_containers::proof::ChildOrigins;
use crate::starmap_parse::{Body, Child};
use crate::starmap_transform::Transform;
use crate::{CatalogError, ExportAccountingError};

impl System<'_> {
    pub(super) fn record_jump(
        &mut self,
        child: &Child,
        path: &WalkPath,
        file: &ContainerFile,
        world: Option<Transform>,
    ) -> Result<(), CatalogError> {
        let Some(guid) = child.record else {
            return Ok(());
        };
        let Some(class) = self.endpoints.and_then(|records| records.get(guid)) else {
            return Ok(());
        };
        let route = path
            .route
            .as_deref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let origins = child
            .origins
            .as_deref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let target = self.gate_for(child, origins)?;
        if class.name == "Anomaly"
            && (matches!(&target.gate, SourceValue::Absent)
                || matches!(&target.gate, SourceValue::Present(gate) if gate.marker_definitively_absent()))
        {
            return Ok(());
        }
        let row = JumpOccurrence {
            root_entry: self.root_entry,
            root_inputs: &self.root,
            system: &self.name,
            route,
            container_route: path.parent_instance.as_deref(),
            local: path.local,
            world,
            record_guid: CatalogGuid(guid),
            class,
            child: origins,
            target: child.external.as_ref(),
        }
        .project(self.capture, file, target)?;
        self.read.jump_endpoints.push(row);
        Ok(())
    }

    fn gate_for(
        &self,
        child: &Child,
        origins: &ChildOrigins,
    ) -> Result<GateResolution, CatalogError> {
        let Some(target) = &child.external else {
            return Ok(GateResolution {
                gate: SourceValue::Absent,
                candidates: self.capture.source_uses().value(
                    SourceValue::Absent,
                    &[origins.external_selector()?],
                    true,
                ),
                proof: vec![origins.external_selector()?],
            });
        };
        let (mut proof, complete) = target.selector_inputs()?;
        let entries = self.index.endpoint_candidates(target.as_str());
        let references = self.target_references(entries, &mut proof)?;
        let candidates = self.capture.source_uses().value(
            SourceValue::Present(references),
            &proof,
            complete && !entries.is_empty(),
        );
        let gate = match entries {
            [entry] => match self.bodies.get(entry) {
                Some(Body::Parsed {
                    gate: Some(gate),
                    gate_proof: Some(original),
                    ..
                }) => {
                    proof.push(original.selected()?);
                    SourceValue::Present(gate.as_ref().clone())
                }
                _ => SourceValue::Unreadable,
            },
            [] => SourceValue::Unreadable,
            _ => SourceValue::Invalid,
        };
        Ok(GateResolution {
            gate,
            candidates,
            proof,
        })
    }

    fn target_references(
        &self,
        indices: &[usize],
        proof: &mut Vec<SourceUse>,
    ) -> Result<Vec<SourcedValue<PhysicalEntryRef>>, CatalogError> {
        indices
            .iter()
            .map(|&index| {
                let entry = self
                    .index
                    .entries
                    .get(index)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                let source = EntrySource::outer(self.capture, self.archive, entry)?;
                let reference = source.receipt().physical_reference(self.capture)?;
                proof.extend(reference.contributions().0.iter().cloned());
                Ok(reference.into_cell(self.capture.source_uses()))
            })
            .collect()
    }
}

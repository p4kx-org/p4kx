// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceValue};

use super::{System, WalkPath};
use crate::guid_text::CatalogGuid;
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::jump_endpoint::JumpOccurrence;
use crate::starmap_containers::jump_gate::JumpGate;
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
        let (gate, target_proof) = self.gate_for(child, origins)?;
        if class.name == "Anomaly"
            && (matches!(&gate, SourceValue::Absent)
                || matches!(&gate, SourceValue::Present(gate) if !gate.has_marker()))
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
        .project(self.capture, file, gate, &target_proof)?;
        self.read.jump_endpoints.push(row);
        Ok(())
    }

    fn gate_for(
        &self,
        child: &Child,
        origins: &ChildOrigins,
    ) -> Result<(SourceValue<JumpGate>, Vec<SourceUse>), CatalogError> {
        let Some(target) = &child.external else {
            return Ok((SourceValue::Absent, vec![origins.external_selector()?]));
        };
        let (mut inputs, _) = target.selector_inputs()?;
        if let Some(Body::Parsed {
            gate: Some(gate),
            gate_proof: Some(proof),
            ..
        }) = self
            .index
            .find(target.as_str())
            .and_then(|entry| self.bodies.get(&entry))
        {
            inputs.push(proof.selected()?);
            return Ok((SourceValue::Present(gate.as_ref().clone()), inputs));
        }
        Ok((SourceValue::Unreadable, inputs))
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceLocation;

/// The original position of the geometry component in an Entity Record.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GeometryComponent {
    pub(crate) record: u32,
    pub(crate) ordinal: usize,
}

#[derive(Debug)]
pub(super) struct Position {
    pub(super) owner: GeometryComponent,
    pub(super) slots: Box<[usize]>,
}

impl Position {
    pub(super) fn at(&self, member: &str) -> SourceLocation {
        let mut path = format!("Components[{}].Geometry", self.owner.ordinal);
        for slot in &self.slots {
            path.push_str(".SubGeometry[");
            path.push_str(&slot.to_string());
            path.push(']');
        }
        path.push_str(member);
        SourceLocation::DataForge {
            record_index: self.owner.record,
            field: path,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GeometrySelectors {
    pub(super) model: Option<Arc<Position>>,
    pub(super) material: Option<Arc<Position>>,
}

impl GeometrySelectors {
    pub(super) fn of(owner: GeometryComponent, slots: &[usize], has_material: bool) -> Self {
        let position = Arc::new(Position {
            owner,
            slots: slots.into(),
        });
        Self {
            material: has_material.then(|| Arc::clone(&position)),
            model: Some(position),
        }
    }

    pub(crate) fn model_field(&self) -> Option<SourceLocation> {
        Some(self.model.as_ref()?.at(".Geometry.Geometry.path"))
    }

    pub(crate) fn material_field(&self) -> Option<SourceLocation> {
        Some(self.material.as_ref()?.at(".Geometry.Material.path"))
    }
}

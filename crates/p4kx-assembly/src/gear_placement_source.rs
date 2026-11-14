// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceLocation;

use crate::{EntityNode, LandingGear};

#[derive(Debug, Clone, Copy)]
pub(crate) struct GearDeclaration {
    pub(crate) record: u32,
    pub(crate) ordinal: usize,
    pub(crate) authored_bone: bool,
}

impl GearDeclaration {
    fn field(self, field: &str) -> SourceLocation {
        SourceLocation::DataForge {
            record_index: self.record,
            field: format!("gears[{}].{field}", self.ordinal),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SystemSelection {
    record: u32,
    component: usize,
}

/// The original DataForge selectors used to resolve and place a landing gear.
#[derive(Debug, Clone, Copy)]
pub struct GearPlacementSource {
    declaration: GearDeclaration,
    system: Option<SystemSelection>,
    read_bone: bool,
}

impl GearPlacementSource {
    pub(crate) fn of(entity: &EntityNode, gear: &LandingGear, read_bone: bool) -> Self {
        let system = entity
            .record_index()
            .zip(entity.landing_system_component)
            .map(|(record, component)| SystemSelection { record, component });
        Self {
            declaration: gear.source,
            system,
            read_bone,
        }
    }

    /// Returns the authored selectors that resolved this gear's geometry.
    pub fn model_selectors(&self) -> impl Iterator<Item = SourceLocation> {
        let system = self.system.map(|selection| SourceLocation::DataForge {
            record_index: selection.record,
            field: format!("Components[{}].landingSystem", selection.component),
        });
        [system, Some(self.declaration.field("geometry.path"))]
            .into_iter()
            .flatten()
    }

    /// Returns the authored bone name when placement evaluated it against the parent model.
    #[must_use]
    pub fn bone_selector(&self) -> Option<SourceLocation> {
        (self.read_bone && self.declaration.authored_bone).then(|| self.declaration.field("bone"))
    }
}

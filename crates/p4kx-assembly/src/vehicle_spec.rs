// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{RecordTree, StructValue};

use crate::geometry::AppearanceOnlyVariant;
use crate::loadout_path::PathRead;
use crate::loadout_reference::LoadoutReference;
use crate::target_spec::{self, TargetSpec};
use crate::{LoadoutPath, LoadoutPathOperation};

/// The vehicle implementation a Record names and the Modification it selects.
#[derive(Debug, Clone)]
pub(crate) struct VehicleSpec {
    pub(crate) definition: LoadoutPath,
    pub(crate) modification: LoadoutPath,
    pub(crate) class_name: Option<LoadoutPath>,
    pub(crate) landing_system: Option<LoadoutReference>,
    pub(crate) component: usize,
    pub(crate) attach_tags: Box<str>,
    pub(crate) appearance_only: Vec<AppearanceOnlyVariant>,
}

impl PartialEq for VehicleSpec {
    fn eq(&self, other: &Self) -> bool {
        self.definition == other.definition
            && self.modification == other.modification
            && self.landing_system == other.landing_system
            && self.attach_tags == other.attach_tags
            && self.appearance_only == other.appearance_only
    }
}

impl Eq for VehicleSpec {}

pub(super) fn read_vehicle(
    component: &StructValue<'_>,
    ordinal: usize,
    tree: &RecordTree<'_>,
    paths: &PathRead<'_>,
    attach_tags: &str,
) -> VehicleSpec {
    VehicleSpec {
        component: ordinal,
        attach_tags: attach_tags.into(),
        appearance_only: Vec::new(),
        definition: paths.field(component, "vehicleDefinition", LoadoutPathOperation::Trim),
        modification: paths.field(component, "modification", LoadoutPathOperation::Trim),
        class_name: paths.record_name(tree),
        landing_system: match component
            .field("landingSystem")
            .map(|value| target_spec::reference_target_at(component, value, paths))
        {
            Some(TargetSpec::Reference(reference)) => Some(reference),
            _ => None,
        },
    }
}

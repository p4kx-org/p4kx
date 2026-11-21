// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::EntityKind;

use super::family::Family;

/// Source declarations needed by one media verification case.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Requirements {
    pub(crate) geometry_lod: Option<u32>,
    pub(crate) item_size: Option<u32>,
    pub(crate) requires_interior: bool,
    pub(crate) requires_audio: bool,
    pub(crate) docked: DockedVehicles,
}

/// Whether a geometry case expands attached docked vehicles.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DockedVehicles {
    Expand,
    Stop,
}

/// A stable technical case and its documented source families.
pub(crate) struct Case {
    pub(crate) label: &'static str,
    pub(crate) preferred: &'static [&'static str],
    pub(crate) families: &'static [&'static str],
    pub(crate) kind: EntityKind,
    pub(crate) requirements: Requirements,
}

impl Case {
    #[must_use]
    pub(crate) const fn from_family(
        label: &'static str,
        family: Family,
        requirements: Requirements,
    ) -> Self {
        let spec = family.spec();
        Self {
            label,
            preferred: spec.preferred,
            families: spec.prefixes,
            kind: spec.kind,
            requirements,
        }
    }

    #[must_use]
    pub(crate) fn accepts_family(&self, class_name: &str) -> bool {
        self.families.iter().any(|family| {
            let Some(prefix) = class_name.get(..family.len()) else {
                return false;
            };
            prefix.eq_ignore_ascii_case(family)
                && (class_name.len() == family.len()
                    || class_name.as_bytes().get(family.len()) == Some(&b'_'))
        })
    }
}

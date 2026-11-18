// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::VehicleAggregateEvidence;
use crate::markup::Element;

pub(super) struct PatchScope<'scope> {
    aggregate: Option<&'scope mut VehicleAggregateEvidence>,
}

impl<'scope> PatchScope<'scope> {
    pub(super) const fn new(aggregate: Option<&'scope mut VehicleAggregateEvidence>) -> Self {
        Self { aggregate }
    }

    pub(super) fn element(&mut self, element: &Element, field: &str) {
        if let Some(aggregate) = &mut self.aggregate {
            aggregate.element(element, field);
        }
    }

    pub(super) fn field(&mut self, element: &Element, name: &str) {
        if let Some(aggregate) = &mut self.aggregate {
            aggregate.field(element, name);
        }
    }
}

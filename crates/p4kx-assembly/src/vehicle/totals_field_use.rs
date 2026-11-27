// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceUseRole;

use crate::{VehicleAttributeEvidence, VehiclePart, VehicleXmlSite};

/// The aggregate that consulted an effective vehicle XML field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VehicleTotalMetric {
    /// The part-mass sum in kilograms.
    Mass,
    /// The structural damage capacity.
    HullHealth,
}

/// Whether a consulted field is present in the effective vehicle declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VehicleTotalFieldPresence {
    /// The effective element has the attribute, including an empty value.
    Present,
    /// The effective attribute or optional `ItemPort` is absent.
    Absent,
}

impl VehicleTotalFieldPresence {
    const fn of(value: Option<&str>) -> Self {
        match value {
            Some(_) => Self::Present,
            None => Self::Absent,
        }
    }
}

/// A consulted field of one retained part and its available original XML evidence.
#[derive(Debug, Clone, Copy)]
pub struct VehicleTotalFieldUse<'vehicle> {
    metric: VehicleTotalMetric,
    role: SourceUseRole,
    presence: VehicleTotalFieldPresence,
    evidence: Option<&'vehicle VehicleAttributeEvidence>,
    absent_child_scope: Option<&'vehicle VehicleXmlSite>,
}

impl<'vehicle> VehicleTotalFieldUse<'vehicle> {
    fn part_field(
        part: &'vehicle VehiclePart,
        metric: VehicleTotalMetric,
        name: &str,
        role: SourceUseRole,
    ) -> Self {
        Self::new(
            metric,
            role,
            VehicleTotalFieldPresence::of(part.attribute(name)),
            part.origins.evidence(name).map(Arc::as_ref),
            None,
        )
    }

    fn new(
        metric: VehicleTotalMetric,
        role: SourceUseRole,
        presence: VehicleTotalFieldPresence,
        evidence: Option<&'vehicle VehicleAttributeEvidence>,
        absent_child_scope: Option<&'vehicle VehicleXmlSite>,
    ) -> Self {
        let mut field = Self {
            metric,
            role,
            presence,
            evidence,
            absent_child_scope,
        };
        if field.site().is_none_or(|site| site.location().is_none()) {
            field.role = SourceUseRole::Attempted;
        }
        field
    }

    /// Returns the aggregate for which the field was consulted.
    #[must_use]
    pub const fn metric(self) -> VehicleTotalMetric {
        self.metric
    }

    /// Returns the field's purpose in the calculation or its attempted read.
    #[must_use]
    pub const fn role(self) -> SourceUseRole {
        self.role
    }

    /// Returns effective presence independently of original coordinate availability.
    #[must_use]
    pub const fn presence(self) -> VehicleTotalFieldPresence {
        self.presence
    }

    /// Borrows the original attribute or element scope consulted for this field.
    #[must_use]
    pub const fn site(self) -> Option<&'vehicle VehicleXmlSite> {
        match self.evidence {
            Some(evidence) => Some(evidence.original()),
            None => self.absent_child_scope,
        }
    }

    /// Borrows the field and its captured assignment selectors when available.
    #[must_use]
    pub const fn evidence(self) -> Option<&'vehicle VehicleAttributeEvidence> {
        self.evidence
    }
}

pub(super) fn observe_mass<'vehicle>(
    part: &'vehicle VehiclePart,
    summed: bool,
    observe: &mut impl FnMut(VehicleTotalFieldUse<'vehicle>),
) {
    let metric = VehicleTotalMetric::Mass;
    observe(VehicleTotalFieldUse::part_field(
        part,
        metric,
        "mass",
        numeric_role(summed),
    ));
    observe(VehicleTotalFieldUse::part_field(
        part,
        metric,
        "skipPart",
        SourceUseRole::Selector,
    ));
}

pub(super) fn observe_hull<'vehicle>(
    part: &'vehicle VehiclePart,
    positive_damage: bool,
    summed: bool,
    observe: &mut impl FnMut(VehicleTotalFieldUse<'vehicle>),
) {
    let metric = VehicleTotalMetric::HullHealth;
    observe(VehicleTotalFieldUse::part_field(
        part,
        metric,
        "damageMax",
        numeric_role(summed),
    ));
    observe(VehicleTotalFieldUse::part_field(
        part,
        metric,
        "skipPart",
        SourceUseRole::Selector,
    ));
    if positive_damage {
        let (presence, evidence, absent_child_scope) = part.item_port.as_ref().map_or_else(
            || {
                (
                    VehicleTotalFieldPresence::Absent,
                    None,
                    part.origins.element(),
                )
            },
            |item_port| {
                (
                    VehicleTotalFieldPresence::of(item_port.flags.as_deref()),
                    item_port.flags_evidence(),
                    None,
                )
            },
        );
        observe(VehicleTotalFieldUse::new(
            metric,
            SourceUseRole::Selector,
            presence,
            evidence,
            absent_child_scope,
        ));
    }
}

const fn numeric_role(summed: bool) -> SourceUseRole {
    if summed {
        SourceUseRole::Conversion
    } else {
        SourceUseRole::Selector
    }
}

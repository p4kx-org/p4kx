// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::read_text_number;

#[path = "totals_field_use.rs"]
mod field_use;

pub use field_use::{VehicleTotalFieldPresence, VehicleTotalFieldUse, VehicleTotalMetric};

use super::{ValueOrigin, VehicleImplementation, VehiclePart};

/// How many values of a sum came from each [`ValueOrigin`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OriginTally {
    base: usize,
    patch: usize,
    elem: usize,
}

impl OriginTally {
    const fn add(&mut self, origin: ValueOrigin) {
        let slot = match origin {
            ValueOrigin::Base => &mut self.base,
            ValueOrigin::Patch => &mut self.patch,
            ValueOrigin::Elem => &mut self.elem,
        };
        *slot = slot.saturating_add(1);
    }

    /// Returns how many values came from `origin`.
    #[must_use]
    pub const fn count(&self, origin: ValueOrigin) -> usize {
        match origin {
            ValueOrigin::Base => self.base,
            ValueOrigin::Patch => self.patch,
            ValueOrigin::Elem => self.elem,
        }
    }

    /// Returns how many values were counted.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.base
            .saturating_add(self.patch)
            .saturating_add(self.elem)
    }
}

/// A part that destroys or detaches at a share of its hit points.
#[derive(Debug, Clone, PartialEq)]
pub struct PartThreshold {
    part: String,
    health: f64,
    damage: f64,
    origin: ValueOrigin,
}

impl PartThreshold {
    /// Returns the part's name.
    #[must_use]
    pub fn part(&self) -> &str {
        &self.part
    }

    /// Returns the part's `damageMax`.
    #[must_use]
    pub const fn health(&self) -> f64 {
        self.health
    }

    /// Returns the damage at which the part destroys the vehicle or detaches.
    #[must_use]
    pub const fn damage(&self) -> f64 {
        self.damage
    }

    /// Returns where the part's `damageMax` comes from.
    #[must_use]
    pub const fn origin(&self) -> ValueOrigin {
        self.origin
    }
}

/// A number attribute of a part that holds text but no number, with its raw text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableValue {
    part: String,
    attribute: &'static str,
    raw: String,
}

impl UnreadableValue {
    /// Returns the part's name.
    #[must_use]
    pub fn part(&self) -> &str {
        &self.part
    }

    /// Returns the attribute's name, e.g. `mass`.
    #[must_use]
    pub const fn attribute(&self) -> &'static str {
        self.attribute
    }

    /// Returns the value as stored.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }
}

/// A number attribute of a part of which only a prefix was read, with its raw text, e.g. `12,5x`
/// read as 12. The value enters a sum or a threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialValue {
    part: String,
    attribute: &'static str,
    raw: String,
    read: f64,
}

impl PartialValue {
    /// Returns the part's name.
    #[must_use]
    pub fn part(&self) -> &str {
        &self.part
    }

    /// Returns the attribute's name, e.g. `mass`.
    #[must_use]
    pub const fn attribute(&self) -> &'static str {
        self.attribute
    }

    /// Returns the value as stored.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the number that was read from it.
    #[must_use]
    pub const fn read(&self) -> f64 {
        self.read
    }
}

/// The sums over the parts of a vehicle implementation: mass, hull hit points and the thresholds
/// at which parts destroy the vehicle or detach (ADR-0061).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VehicleTotals {
    mass: f64,
    mass_parts: usize,
    mass_origins: OriginTally,
    health: f64,
    health_parts: usize,
    health_origins: OriginTally,
    destruction: Vec<PartThreshold>,
    detach: Vec<PartThreshold>,
    unreadable: Vec<UnreadableValue>,
    partial: Vec<PartialValue>,
    cleaned_mass: usize,
    cleaned_damage: usize,
}

impl VehicleTotals {
    /// Returns the sum of the `mass` of every part, in kilograms.
    #[must_use]
    pub const fn mass(&self) -> f64 {
        self.mass
    }

    /// Returns how many parts state a mass.
    #[must_use]
    pub const fn mass_parts(&self) -> usize {
        self.mass_parts
    }

    /// Returns where the masses summed came from.
    #[must_use]
    pub const fn mass_origins(&self) -> OriginTally {
        self.mass_origins
    }

    /// Returns the sum of the `damageMax` of the structural parts: every part with a positive
    /// `damageMax` except those holding an item port flagged `uneditable` or `invisible`.
    #[must_use]
    pub const fn health(&self) -> f64 {
        self.health
    }

    /// Returns how many parts the health sums.
    #[must_use]
    pub const fn health_parts(&self) -> usize {
        self.health_parts
    }

    /// Returns where the `damageMax` values summed came from.
    #[must_use]
    pub const fn health_origins(&self) -> OriginTally {
        self.health_origins
    }

    /// Returns the parts that destroy the vehicle when they take `damageRatioMin` of their
    /// `damageMax`, in part order.
    #[must_use]
    pub fn destruction(&self) -> &[PartThreshold] {
        &self.destruction
    }

    /// Returns the parts that detach when they take `detachRatio` of their `damageMax` and have
    /// no destroying behaviour, in part order.
    #[must_use]
    pub fn detach(&self) -> &[PartThreshold] {
        &self.detach
    }

    /// Returns the `mass`, `damageMax` and `detachRatio` values that hold text but no number;
    /// they are left out of the sums, never counted as 0.
    #[must_use]
    pub fn unreadable(&self) -> &[UnreadableValue] {
        &self.unreadable
    }

    /// Returns the `mass`, `damageMax` and `detachRatio` values that enter a sum or a threshold
    /// and of which only a numeric prefix was read, because other text than a unit (`d`, `f`,
    /// `kg`) follows it.
    #[must_use]
    pub fn partial(&self) -> &[PartialValue] {
        &self.partial
    }

    /// Returns how many numbers read carried invisible format characters that were dropped.
    #[must_use]
    pub const fn cleaned(&self) -> usize {
        self.cleaned_mass.saturating_add(self.cleaned_damage)
    }

    /// Returns how many `mass` numbers needed their format characters dropped.
    #[must_use]
    pub const fn cleaned_mass(&self) -> usize {
        self.cleaned_mass
    }

    /// Returns how many `damageMax` and `detachRatio` numbers needed their format characters
    /// dropped.
    #[must_use]
    pub const fn cleaned_damage(&self) -> usize {
        self.cleaned_damage
    }
}

impl VehicleImplementation {
    /// Returns the sums over the parts: mass, hull hit points and the destruction and detach
    /// thresholds.
    #[must_use]
    pub fn totals(&self) -> VehicleTotals {
        self.totals_with_field_uses(|_| {})
    }

    /// Returns the same totals while visiting the mass and hull fields of retained parts.
    ///
    /// Field use evidence covers the consulted fields. Aggregate membership and file selection
    /// require their own evidence.
    #[must_use]
    pub fn totals_with_field_uses<'vehicle>(
        &'vehicle self,
        mut observe: impl FnMut(VehicleTotalFieldUse<'vehicle>),
    ) -> VehicleTotals {
        let mut totals = VehicleTotals::default();
        for part in self.iter() {
            add_part(&mut totals, part, &mut observe);
        }
        totals
    }
}

fn add_part<'vehicle>(
    totals: &mut VehicleTotals,
    part: &'vehicle VehiclePart,
    observe: &mut impl FnMut(VehicleTotalFieldUse<'vehicle>),
) {
    let mass = number_of(totals, part, "mass", part.mass);
    field_use::observe_mass(part, mass.is_some(), observe);
    if let Some(mass) = mass {
        totals.mass += mass;
        totals.mass_parts = totals.mass_parts.saturating_add(1);
        totals.mass_origins.add(part.origin_of("mass"));
        note_partial(totals, part, "mass", mass);
    }
    let detach = number_of(totals, part, "detachRatio", part.detach_ratio);
    let health = number_of(totals, part, "damageMax", part.damage_max).filter(|h| *h > 0.0);
    let in_hull = health.is_some() && counts_as_hull(part);
    field_use::observe_hull(part, health.is_some(), in_hull, observe);
    let Some(health) = health else {
        return;
    };
    let origin = part.origin_of("damageMax");
    if in_hull {
        totals.health += health;
        totals.health_parts = totals.health_parts.saturating_add(1);
        totals.health_origins.add(origin);
    }
    let mut damage_used = in_hull;
    let threshold = |damage: f64| PartThreshold {
        part: part.name.clone(),
        health,
        damage,
        origin,
    };
    if let Some(ratio) = part.destroy_ratio {
        let damage = ratio * health;
        if damage > 0.0 {
            totals.destruction.push(threshold(damage));
            damage_used = true;
        }
    } else if let Some(ratio) = detach.filter(|ratio| *ratio > 0.0) {
        totals.detach.push(threshold(ratio * health));
        damage_used = true;
        note_partial(totals, part, "detachRatio", ratio);
    }
    if damage_used {
        note_partial(totals, part, "damageMax", health);
    }
}

/// Records the attribute `name` of `part` as partly read when its raw text has a remainder after
/// the number `read`. Only values that enter a sum or a threshold are recorded.
fn note_partial(totals: &mut VehicleTotals, part: &VehiclePart, name: &'static str, read: f64) {
    if let Some(raw) = part.attribute(name)
        && read_text_number(raw).is_some_and(|number| number.is_partial())
    {
        totals.partial.push(PartialValue {
            part: part.name.clone(),
            attribute: name,
            raw: raw.to_owned(),
            read,
        });
    }
}

fn counts_as_hull(part: &VehiclePart) -> bool {
    part.item_port
        .as_ref()
        .is_none_or(|port| !(port.has_flag("uneditable") || port.has_flag("invisible")))
}

/// Returns `parsed`, or records the attribute `name` of `part` as unreadable when it holds text
/// that is no number; counts a number that needed its format characters dropped.
fn number_of(
    totals: &mut VehicleTotals,
    part: &VehiclePart,
    name: &'static str,
    parsed: Option<f64>,
) -> Option<f64> {
    let raw = part.attribute(name).filter(|raw| !raw.trim().is_empty())?;
    match parsed {
        Some(_) => {
            if read_text_number(raw).is_some_and(|number| number.was_cleaned()) {
                let counter = if name == "mass" {
                    &mut totals.cleaned_mass
                } else {
                    &mut totals.cleaned_damage
                };
                *counter = counter.saturating_add(1);
            }
        }
        None => totals.unreadable.push(UnreadableValue {
            part: part.name.clone(),
            attribute: name,
            raw: raw.to_owned(),
        }),
    }
    parsed
}

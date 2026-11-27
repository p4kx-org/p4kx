// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};

use super::{
    ProjectileCombatInputs, WeaponAction, WeaponActionKind, WeaponActionRecoil, WeaponLauncher,
    WeaponModeKind,
};

impl<'source> WeaponAction<'source> {
    /// Borrows the original local value ordinal within the physical Record.
    #[must_use]
    pub const fn source_ordinal(&self) -> &SourcedValue<DecimalU64> {
        &self.source_value_ordinal
    }

    /// Borrows the original concrete action type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the original recoil links and interval.
    #[must_use]
    pub const fn recoil(&self) -> &WeaponActionRecoil<'source> {
        &self.recoil
    }

    /// Returns the action family used to select combat derivations.
    #[must_use]
    pub const fn mode_kind(&self) -> WeaponModeKind {
        match self.kind {
            WeaponActionKind::Single(_) | WeaponActionKind::Rapid(_) => WeaponModeKind::SimpleShot,
            WeaponActionKind::Burst(_) => WeaponModeKind::Burst,
            WeaponActionKind::Beam(_) => WeaponModeKind::Beam,
            WeaponActionKind::Charged(_) => WeaponModeKind::Charge,
            WeaponActionKind::Conditional(_) | WeaponActionKind::DynamicConditional(_) => {
                WeaponModeKind::Conditional
            }
            _ => WeaponModeKind::Other,
        }
    }

    /// Borrows the sourced operands when a single or rapid action declares a projectile launcher.
    #[must_use]
    pub fn combat_inputs(&self) -> Option<ProjectileCombatInputs<'_>> {
        let shot = match &self.kind {
            WeaponActionKind::Single(shot) => shot.as_ref(),
            WeaponActionKind::Rapid(rapid) => &rapid.shot,
            _ => return None,
        };
        let WeaponLauncher::Projectile { settings } = shot.launch_params.value()? else {
            return None;
        };
        Some(ProjectileCombatInputs {
            fire_rate: &shot.fire_rate,
            pellet_count: settings.pellet_count(),
            damage_multiplier: settings.damage_multiplier(),
        })
    }
}

impl<'source> WeaponAction<'source> {
    pub(crate) const fn kind(&self) -> &WeaponActionKind<'source> {
        &self.kind
    }
}

impl WeaponActionKind<'_> {
    pub(crate) fn fire_rate(&self) -> Option<&SourcedValue<i32>> {
        match self {
            Self::Single(shot) => Some(&shot.fire_rate),
            Self::Rapid(rapid) => Some(&rapid.shot.fire_rate),
            Self::Burst(burst) => Some(&burst.shot.fire_rate),
            _ => None,
        }
    }
}

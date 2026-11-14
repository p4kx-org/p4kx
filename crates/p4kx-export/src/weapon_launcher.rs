// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

const BASE: &str = "SLauncherBase";

/// The original launcher alternative and its authored settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum WeaponLauncher<'source> {
    Base {
        authored: SourceArenaOutput<'source>,
    },
    Dummy {
        authored: SourceArenaOutput<'source>,
    },
    Projectile {
        #[serde(flatten)]
        settings: Box<ProjectileLauncher<'source>>,
    },
    Missile {
        #[serde(flatten)]
        settings: Box<MissileLauncher<'source>>,
    },
    Throwable {
        #[serde(flatten)]
        settings: Box<ThrowableLauncher<'source>>,
    },
    Authored {
        #[serde(rename = "sourceType")]
        source_type: SourcedValue<&'source str>,
        original: SourceArenaOutput<'source>,
    },
}

impl<'source> WeaponLauncher<'source> {
    pub(super) fn read_field(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        source.strong_pointer_family_to("launchParams", BASE, notes, Self::read)
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            BASE => Self::Base {
                authored: source.authored_members()?,
            },
            "SDummyLauncher" => Self::Dummy {
                authored: source.authored_members()?,
            },
            "SProjectileLauncher" => Self::Projectile {
                settings: Box::new(ProjectileLauncher::read(source, notes)?),
            },
            "SMissileLauncher" => Self::Missile {
                settings: Box::new(MissileLauncher::read(source, notes)?),
            },
            "SThrowableLauncher" => Self::Throwable {
                settings: Box::new(ThrowableLauncher::read(source, notes)?),
            },
            _ => Self::Authored {
                source_type: source.source_type_value()?,
                original: source.authored_members()?,
            },
        })
    }
}

/// Projectile launcher costs, selectors and other stored inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileLauncher<'source> {
    fire_helper: SourcedValue<&'source str>,
    muzzle_helper: SourcedValue<&'source str>,
    ammo_cost: SourcedValue<i32>,
    heat_cost: SourcedValue<ResourceFloat32>,
    pellet_count: SourcedValue<i32>,
    damage_multiplier: SourcedValue<ResourceFloat32>,
    sound_radius: SourcedValue<ResourceFloat32>,
    spread_params: SourcedValue<LauncherSpread<'source>>,
    projectile_type: SourcedValue<&'source str>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ProjectileLauncher<'source> {
    pub(super) const fn projectile_type(&self) -> &SourcedValue<&'source str> {
        &self.projectile_type
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let fire_helper = source.borrowed_string("fireHelper", notes)?;
        let muzzle_helper = source.borrowed_string("muzzleHelper", notes)?;
        let ammo_cost = source.int32("ammoCost", notes)?;
        let heat_cost = source.single("heatCost", notes)?;
        let pellet_count = source.int32("pelletCount", notes)?;
        let damage_multiplier = source.single("damageMultiplier", notes)?;
        let sound_radius = source.single("soundRadius", notes)?;
        let spread_params = source.inline_class(
            "spreadParams",
            &["SSpreadParams"],
            notes,
            LauncherSpread::read,
        )?;
        let projectile_type =
            source.borrowed_enum_to("projectileType", "EProjectileType", notes)?;
        let authored = source.authored_members_except([
            ("fireHelper", fire_helper.state()),
            ("muzzleHelper", muzzle_helper.state()),
            ("ammoCost", ammo_cost.state()),
            ("heatCost", heat_cost.state()),
            ("pelletCount", pellet_count.state()),
            ("damageMultiplier", damage_multiplier.state()),
            ("soundRadius", sound_radius.state()),
            ("spreadParams", spread_params.state()),
            ("projectileType", projectile_type.state()),
        ])?;
        Ok(Self {
            fire_helper,
            muzzle_helper,
            ammo_cost,
            heat_cost,
            pellet_count,
            damage_multiplier,
            sound_radius,
            spread_params,
            projectile_type,
            authored,
        })
    }
}

/// The original spread words.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LauncherSpread<'source> {
    min: SourcedValue<ResourceFloat32>,
    max: SourcedValue<ResourceFloat32>,
    first_attack: SourcedValue<ResourceFloat32>,
    attack: SourcedValue<ResourceFloat32>,
    decay: SourcedValue<ResourceFloat32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> LauncherSpread<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let min = source.single("min", notes)?;
        let max = source.single("max", notes)?;
        let first_attack = source.single("firstAttack", notes)?;
        let attack = source.single("attack", notes)?;
        let decay = source.single("decay", notes)?;
        let authored = source.authored_members_except([
            ("min", min.state()),
            ("max", max.state()),
            ("firstAttack", first_attack.state()),
            ("attack", attack.state()),
            ("decay", decay.state()),
        ])?;
        Ok(Self {
            min,
            max,
            first_attack,
            attack,
            decay,
            authored,
        })
    }
}

/// The original missile-launch settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissileLauncher<'source> {
    sound_radius: SourcedValue<ResourceFloat32>,
    detach_velocity_right: SourcedValue<ResourceFloat32>,
    detach_velocity_forward: SourcedValue<ResourceFloat32>,
    detach_velocity_up: SourcedValue<ResourceFloat32>,
    detach_velocity_roll: SourcedValue<ResourceFloat32>,
    can_fire_without_lock: SourcedValue<bool>,
    show_hit_indicator: SourcedValue<bool>,
    ignite_on_pylon: SourcedValue<bool>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> MissileLauncher<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let sound_radius = source.single("soundRadius", notes)?;
        let detach_velocity_right = source.single("detachVelocityRight", notes)?;
        let detach_velocity_forward = source.single("detachVelocityForward", notes)?;
        let detach_velocity_up = source.single("detachVelocityUp", notes)?;
        let detach_velocity_roll = source.single("detachVelocityRoll", notes)?;
        let can_fire_without_lock = source.boolean("canFireWithoutLock", notes)?;
        let show_hit_indicator = source.boolean("showHitIndicator", notes)?;
        let ignite_on_pylon = source.boolean("igniteOnPylon", notes)?;
        let authored = source.authored_members_except([
            ("soundRadius", sound_radius.state()),
            ("detachVelocityRight", detach_velocity_right.state()),
            ("detachVelocityForward", detach_velocity_forward.state()),
            ("detachVelocityUp", detach_velocity_up.state()),
            ("detachVelocityRoll", detach_velocity_roll.state()),
            ("canFireWithoutLock", can_fire_without_lock.state()),
            ("showHitIndicator", show_hit_indicator.state()),
            ("igniteOnPylon", ignite_on_pylon.state()),
        ])?;
        Ok(Self {
            sound_radius,
            detach_velocity_right,
            detach_velocity_forward,
            detach_velocity_up,
            detach_velocity_roll,
            can_fire_without_lock,
            show_hit_indicator,
            ignite_on_pylon,
            authored,
        })
    }
}

/// The original throw mode and sound radius.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ThrowableLauncher<'source> {
    throw_mode: SourcedValue<&'source str>,
    sound_radius: SourcedValue<ResourceFloat32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ThrowableLauncher<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let throw_mode = source.borrowed_enum_to("throwMode", "EThrowMode", notes)?;
        let sound_radius = source.single("soundRadius", notes)?;
        let authored = source.authored_members_except([
            ("throwMode", throw_mode.state()),
            ("soundRadius", sound_radius.state()),
        ])?;
        Ok(Self {
            throw_mode,
            sound_radius,
            authored,
        })
    }
}

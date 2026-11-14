// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, RecordTree, Value};

use super::authored_fields::Observed;
use super::catalog_ammo_bindings::{self, AmmoBinding};
use super::catalog_ammo_containers::{self, AmmoContainer};
use super::catalog_durability::{self, DurabilityComponent, DurabilityRows, DurabilityTables};
use super::catalog_emp::{self, Emp};
use super::catalog_explosives::catalog_missile_configuration::{self, MissileConfiguration};
use super::catalog_explosives::{self, Explosive, ExplosiveComponent};
use super::catalog_hacking_chips::{self, HackingChip, HackingComponent};
use super::catalog_jump_drive::{self, JumpDrive};
use super::catalog_quantum_drive::{self, QuantumDrive};
use super::catalog_quantum_interdiction::{self, QuantumInterdictionGenerator};
use super::catalog_shield_generator::{self, ShieldGenerator};
use super::catalog_weapon_actions::{self, WeaponActions};
use super::catalog_weapon_magazine_slots::{self, WeaponMagazineSlots};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[path = "catalog_weapon_magazine_resolution.rs"]
pub(crate) mod resolution;

use self::resolution::WeaponMagazineResolution;

#[derive(Default)]
pub(crate) struct ItemComponentRows<'source> {
    pub(crate) durability: DurabilityRows<'source>,
    pub(crate) emp: Vec<Emp<'source>>,
    pub(crate) explosives: Vec<Explosive<'source>>,
    pub(crate) hacking_chips: Vec<HackingChip<'source>>,
    pub(crate) ammo_containers: Vec<AmmoContainer<'source>>,
    pub(crate) ammo_bindings: Vec<AmmoBinding<'source>>,
    pub(crate) weapon_actions: Vec<WeaponActions<'source>>,
    pub(crate) missile_configurations: Vec<MissileConfiguration<'source>>,
    pub(crate) jump_drives: Vec<JumpDrive<'source>>,
    pub(crate) quantum_drives: Vec<QuantumDrive<'source>>,
    pub(crate) quantum_interdiction: Vec<QuantumInterdictionGenerator<'source>>,
    pub(crate) shield_generators: Vec<ShieldGenerator<'source>>,
    pub(crate) magazine_resolutions: Vec<WeaponMagazineResolution>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> ItemComponentRows<'source> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.durability.extend(other.durability);
        self.emp.extend(other.emp);
        self.explosives.extend(other.explosives);
        self.hacking_chips.extend(other.hacking_chips);
        self.ammo_containers.extend(other.ammo_containers);
        self.ammo_bindings.extend(other.ammo_bindings);
        self.weapon_actions.extend(other.weapon_actions);
        self.missile_configurations
            .extend(other.missile_configurations);
        self.jump_drives.extend(other.jump_drives);
        self.quantum_drives.extend(other.quantum_drives);
        self.quantum_interdiction.extend(other.quantum_interdiction);
        self.shield_generators.extend(other.shield_generators);
        self.magazine_resolutions.extend(other.magazine_resolutions);
        self.numbers.extend(other.numbers);
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> ItemComponentTables<'source> {
        ItemComponentTables {
            durability: catalog_durability::tables(forge, self.durability),
            emp: catalog_emp::table(forge, self.emp),
            explosives: catalog_explosives::table(forge, self.explosives),
            hacking_chips: catalog_hacking_chips::table(forge, self.hacking_chips),
            ammo_containers: catalog_ammo_containers::table(forge, self.ammo_containers),
            ammo_bindings: catalog_ammo_bindings::table(forge, self.ammo_bindings),
            weapon_actions: catalog_weapon_actions::table(forge, self.weapon_actions),
            missile_configurations: catalog_missile_configuration::table(
                forge,
                self.missile_configurations,
            ),
            jump_drives: catalog_jump_drive::table(forge, self.jump_drives),
            quantum_drives: catalog_quantum_drive::table(forge, self.quantum_drives),
            quantum_interdiction: catalog_quantum_interdiction::table(
                forge,
                self.quantum_interdiction,
            ),
            shield_generators: catalog_shield_generator::table(forge, self.shield_generators),
            magazine_resolutions: resolution::table(forge, self.magazine_resolutions),
        }
    }
}

pub(crate) struct ItemComponentTables<'source> {
    pub(crate) durability: DurabilityTables<'source>,
    pub(crate) emp: CatalogTable<Emp<'source>>,
    pub(crate) explosives: CatalogTable<Explosive<'source>>,
    pub(crate) hacking_chips: CatalogTable<HackingChip<'source>>,
    pub(crate) ammo_containers: CatalogTable<AmmoContainer<'source>>,
    pub(crate) ammo_bindings: CatalogTable<AmmoBinding<'source>>,
    pub(crate) weapon_actions: CatalogTable<WeaponActions<'source>>,
    pub(crate) missile_configurations: CatalogTable<MissileConfiguration<'source>>,
    pub(crate) jump_drives: CatalogTable<JumpDrive<'source>>,
    pub(crate) quantum_drives: CatalogTable<QuantumDrive<'source>>,
    pub(crate) quantum_interdiction: CatalogTable<QuantumInterdictionGenerator<'source>>,
    pub(crate) shield_generators: CatalogTable<ShieldGenerator<'source>>,
    pub(crate) magazine_resolutions: CatalogTable<WeaponMagazineResolution>,
}

#[derive(Default)]
pub(crate) struct ItemComponentsRead<'source> {
    pub(crate) rows: ItemComponentRows<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) legacy_health: Option<f32>,
}

impl<'source> ItemComponentsRead<'source> {
    fn push(
        &mut self,
        kind: Component,
        owner: ComponentOwner,
        observed: Observed<'_, '_, '_, 'source>,
    ) -> Result<(), CatalogError> {
        match kind {
            Component::Durability(kind) => {
                if let Some(legacy) =
                    self.rows
                        .durability
                        .push(kind, owner, observed, &mut self.notes)?
                {
                    self.legacy_health = Some(legacy);
                }
            }
            Component::Emp => {
                self.rows
                    .emp
                    .push(Emp::read(owner, observed, &mut self.notes)?);
            }
            Component::Explosive(kind) => self.push_explosive(kind, owner, observed)?,
            Component::AmmoContainer => {
                self.rows.ammo_containers.push(AmmoContainer::read(
                    owner.clone(),
                    observed,
                    &mut self.notes,
                )?);
                self.rows.ammo_bindings.push(AmmoBinding::container(
                    owner,
                    observed,
                    &mut self.notes,
                )?);
            }
            Component::Weapon => self.push_weapon(owner, observed)?,
            Component::JumpDrive => {
                self.rows
                    .jump_drives
                    .push(JumpDrive::read(owner, observed, &mut self.notes)?);
            }
            Component::QuantumDrive => {
                let drive = QuantumDrive::read(owner, observed, &mut self.notes)?;
                self.rows.quantum_drives.push(drive);
            }
            Component::QuantumInterdictionGenerator => {
                let generator =
                    QuantumInterdictionGenerator::read(owner, observed, &mut self.notes)?;
                self.rows.quantum_interdiction.push(generator);
            }
            Component::ShieldGenerator => self.rows.shield_generators.push(ShieldGenerator::read(
                owner,
                observed,
                &mut self.notes,
            )?),
            Component::Hacking(kind) => self.rows.hacking_chips.push(HackingChip::read(
                kind,
                owner,
                observed,
                &mut self.notes,
            )?),
        }
        Ok(())
    }

    fn push_explosive(
        &mut self,
        kind: ExplosiveComponent,
        owner: ComponentOwner,
        observed: Observed<'_, '_, '_, 'source>,
    ) -> Result<(), CatalogError> {
        let missile_owner = matches!(kind, ExplosiveComponent::Missile).then(|| owner.clone());
        let explosive = Explosive::read(kind, owner, observed, &mut self.notes)?;
        if let Some(owner) = missile_owner {
            let ordnance = explosive
                .missile_ordnance()
                .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
            self.rows
                .missile_configurations
                .push(MissileConfiguration::read(
                    owner,
                    observed,
                    ordnance,
                    &mut self.notes,
                )?);
        }
        self.rows.explosives.push(explosive);
        Ok(())
    }

    fn push_weapon(
        &mut self,
        owner: ComponentOwner,
        observed: Observed<'_, '_, '_, 'source>,
    ) -> Result<(), CatalogError> {
        self.rows.weapon_actions.push(WeaponActions::read(
            owner.clone(),
            observed,
            &mut self.notes,
        )?);
        self.rows
            .ammo_bindings
            .push(AmmoBinding::WeaponMagazineSlots {
                value: Box::new(WeaponMagazineSlots::read(owner, observed, &mut self.notes)?),
            });
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Component {
    Durability(DurabilityComponent),
    Emp,
    Explosive(ExplosiveComponent),
    Hacking(HackingComponent),
    AmmoContainer,
    Weapon,
    JumpDrive,
    QuantumDrive,
    QuantumInterdictionGenerator,
    ShieldGenerator,
}

impl Component {
    fn of<'read, 'source>(value: &'read Value<'source>) -> Option<(&'read Value<'source>, Self)> {
        let Value::Struct(source) = value else {
            return None;
        };
        let name = source.type_name();
        match name {
            catalog_ammo_containers::SOURCE_TYPE => return Some((value, Self::AmmoContainer)),
            catalog_emp::SOURCE_TYPE => return Some((value, Self::Emp)),
            catalog_jump_drive::SOURCE_TYPE => return Some((value, Self::JumpDrive)),
            catalog_quantum_drive::SOURCE_TYPE => return Some((value, Self::QuantumDrive)),
            catalog_quantum_interdiction::SOURCE_TYPE => {
                return Some((value, Self::QuantumInterdictionGenerator));
            }
            catalog_shield_generator::SOURCE_TYPE => return Some((value, Self::ShieldGenerator)),
            catalog_weapon_magazine_slots::SOURCE_TYPE => {
                return Some((value, Self::Weapon));
            }
            _ => {}
        }
        let kind = DurabilityComponent::of(name)
            .map(Self::Durability)
            .or_else(|| ExplosiveComponent::of(name).map(Self::Explosive))
            .or_else(|| HackingComponent::of(name).map(Self::Hacking))?;
        Some((value, kind))
    }
}

/// Reads each recognized physical component occurrence using its own admitted Record tree.
pub(crate) fn read<'source, T>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    project: impl FnOnce(
        Option<&OriginalFlowFields<'_, '_, 'source>>,
        &mut ItemComponentsRead<'source>,
    ) -> Result<T, CatalogError>,
) -> Result<(ItemComponentsRead<'source>, T), CatalogError> {
    let mut result = ItemComponentsRead::default();
    let components = match tree.field("Components") {
        Some(Value::Array(components)) => components.as_ref(),
        _ => &[],
    };
    let mut components = components.iter().filter_map(Component::of).peekable();
    if components.peek().is_none() {
        let projected = project(None, &mut result)?;
        return Ok((result, projected));
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());

    for (value, kind) in components {
        let component = originals.fact(value)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let observed = Observed::new(&originals, input, component, &capture, &numbers);
        result.push(kind, owner, observed)?;
    }
    result.rows.numbers = numbers.into_inner();
    let projected = project(Some(&originals), &mut result)?;
    Ok((result, projected))
}

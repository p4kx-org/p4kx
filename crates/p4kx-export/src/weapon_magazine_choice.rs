// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceValueState};

use super::linked_reader::{LinkedAmmunitionReader, LinkedMagazine, LinkedMagazineRead};
use super::route::{self, OwnRoute};
use super::{ContainerAmmoSlots, OriginalFlowFields, ProjectileAmmoRole, Selection};
use crate::CatalogError;
use crate::catalog_v3::catalog_weapon_magazine_slots::{MAGAZINE_TARGET, WeaponMagazineSlots};
use crate::definition_reference::DefinitionStatus;
use crate::export_ledger::ExportLedger;

pub(super) struct LinkedSlots<'source> {
    pub(super) slots: ContainerAmmoSlots<'source>,
    pub(super) evidence: FieldOrigin,
    pub(super) declarations: FieldOrigin,
    pub(super) unique_primary: bool,
    pub(super) unique_secondary: bool,
}

impl LinkedSlots<'_> {
    pub(super) const fn unique(&self, role: ProjectileAmmoRole) -> bool {
        match role {
            ProjectileAmmoRole::Primary => self.unique_primary,
            ProjectileAmmoRole::Secondary => self.unique_secondary,
        }
    }
}

pub(super) enum MagazineChoice<'source> {
    Own,
    NoRoute,
    OtherRoute,
    Indeterminate,
    Dangling,
    WrongType,
    Linked(LinkedMagazine<'source>),
}

impl<'source> MagazineChoice<'source> {
    pub(super) fn read(
        reader: &LinkedAmmunitionReader<'_, 'source>,
        fields: &OriginalFlowFields<'_, '_, 'source>,
        route: &OwnRoute<'_, 'source>,
        magazine: &WeaponMagazineSlots<'source>,
        linked: Option<LinkedMagazineRead<'_, 'source>>,
        ledger: &mut ExportLedger,
    ) -> Result<Self, CatalogError> {
        if matches!(route, OwnRoute::Incomplete(_) | OwnRoute::Competing(_)) {
            return Ok(Self::Indeterminate);
        }
        if !route::magazine_route_fields_unique(fields, magazine)? {
            return Ok(Self::Indeterminate);
        }
        let inputs = magazine.route_inputs();
        if matches!(route, OwnRoute::Unique { .. }) {
            return Ok(if inputs.other_route() {
                Self::OtherRoute
            } else if inputs.permits_own_route() {
                Self::Own
            } else {
                Self::Indeterminate
            });
        }
        if !inputs.permits_linked_route() {
            return Ok(if inputs.has_other_linked_route() {
                Self::OtherRoute
            } else {
                Self::Indeterminate
            });
        }
        let primary = inputs.primary();
        let Some(reference) = primary.value() else {
            return Ok(
                if matches!(
                    primary.state(),
                    SourceValueState::Absent | SourceValueState::Null
                ) {
                    Self::NoRoute
                } else {
                    Self::Indeterminate
                },
            );
        };
        match reference.status {
            DefinitionStatus::Dangling => return Ok(Self::Dangling),
            DefinitionStatus::UnexpectedTargetType => return Ok(Self::WrongType),
            DefinitionStatus::Present => {}
        }
        if reference.target_type.value().copied() != Some(MAGAZINE_TARGET) {
            return Ok(Self::WrongType);
        }
        LinkedMagazine::read(reader, fields, magazine.owner(), linked, reference, ledger)
            .map(Self::Linked)
    }

    pub(super) fn selected<'choice>(
        &'choice self,
        role: ProjectileAmmoRole,
        origins: &mut Vec<&'choice FieldOrigin>,
    ) -> Result<Option<&'choice ContainerAmmoSlots<'source>>, Selection> {
        let result = match self {
            Self::Own => None,
            Self::NoRoute => return Err(Selection::NoAdmittedOwnContainer),
            Self::OtherRoute => return Err(Selection::OtherMagazineRoute),
            Self::Indeterminate => return Err(Selection::IndeterminateMagazineRoute),
            Self::Dangling => return Err(Selection::DanglingMagazine),
            Self::WrongType => return Err(Selection::UnsupportedMagazineTarget),
            Self::Linked(target) => Some(Self::selected_linked(target, role, origins)?),
        };
        Ok(result)
    }

    fn selected_linked<'choice>(
        target: &'choice LinkedMagazine<'source>,
        role: ProjectileAmmoRole,
        origins: &mut Vec<&'choice FieldOrigin>,
    ) -> Result<&'choice ContainerAmmoSlots<'source>, Selection> {
        let (origin, failure) = match target {
            LinkedMagazine::Unique(slots) => {
                origins.extend([&slots.evidence, &slots.declarations]);
                return if slots.unique(role) {
                    Ok(&slots.slots)
                } else {
                    Err(Selection::IndeterminateAmmo)
                };
            }
            LinkedMagazine::SelfReference => return Err(Selection::SelfLinkedMagazine),
            LinkedMagazine::AmbiguousReference(origin) => {
                (origin, Selection::AmbiguousMagazineReference)
            }
            LinkedMagazine::Unreadable(origin) => (origin, Selection::UnreadableMagazine),
            LinkedMagazine::NoContainer(origin) => {
                (origin, Selection::LinkedMagazineWithoutContainer)
            }
            LinkedMagazine::AmbiguousContainer(origin) => {
                (origin, Selection::AmbiguousLinkedMagazineContainer)
            }
            LinkedMagazine::IncompleteComponents(origin) => {
                (origin, Selection::IncompleteLinkedMagazineComponents)
            }
        };
        origins.push(origin);
        Err(failure)
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use super::dependencies::Dependencies;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::{
    mineable_composition_definition, mineable_element_definition, mineable_entity_definition,
};
use crate::mining_locations::{MineableComposition, MineableElement};
use crate::record_observation::RecordObservation;
use crate::resource_type_contract::ResourceDefinition;
use crate::scan_signature::SignatureSlot;
use crate::source_fields::FieldError;
use crate::{CatalogError, Kind, resource_read};

pub(super) trait Projector {
    type Row<'source>;
    const KIND: Kind;
    const LOCALIZED: bool;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError>;
}

pub(super) struct Elements;
pub(super) struct Compositions;
pub(super) struct Entities;

impl Projector for Entities {
    type Row<'source> = mineable_entity_definition::MineableEntity<'source>;
    const KIND: Kind = Kind::MineableEntities;
    const LOCALIZED: bool = false;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError> {
        let unknown = SignatureSlot::unknown();
        let slot = if mineable_entity_definition::needs_signature_slot(tree) {
            dependencies.signature_slot(projection.original(), cancel)?
        } else {
            &unknown
        };
        mineable_entity_definition::read(tree, projection, admission, slot, cancel)
    }
}
pub(super) struct Resources;

impl Projector for Elements {
    type Row<'source> = MineableElement;
    const KIND: Kind = Kind::MineableElements;
    const LOCALIZED: bool = false;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError> {
        mineable_element_definition::read(tree, projection, admission, cancel)
    }
}

impl Projector for Compositions {
    type Row<'source> = MineableComposition;
    const KIND: Kind = Kind::MineableCompositions;
    const LOCALIZED: bool = true;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError> {
        let read = mineable_composition_definition::read(tree, projection, admission, cancel)?;
        Ok((read.value, read.notes))
    }
}

impl Projector for Resources {
    type Row<'source> = ResourceDefinition<'source>;
    const KIND: Kind = Kind::ResourceTypes;
    const LOCALIZED: bool = true;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError> {
        let read =
            resource_read::read_with_memberships(tree, projection, admission, cancel, || {
                dependencies.memberships(tree.guid(), projection.original(), cancel)
            })?;
        Ok((read.value.data, read.notes))
    }
}

pub(super) struct Refining;
pub(super) struct Globals;

impl Projector for Refining {
    type Row<'source> =
        crate::catalog_v3::mining_process_catalog::RefiningProcessDefinition<'source>;
    const KIND: Kind = Kind::RefiningProcesses;
    const LOCALIZED: bool = true;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError> {
        let (prepared, notes) = crate::catalog_v3::mining_process_catalog::read_refining(
            tree, projection, admission, cancel,
        )?;
        Ok((prepared.into_definition(), notes))
    }
}

impl Projector for Globals {
    type Row<'source> =
        crate::catalog_v3::mining_process_catalog::MiningGlobalConfiguration<'source>;
    const KIND: Kind = Kind::MiningGlobalParams;
    const LOCALIZED: bool = false;

    fn read<'source>(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _dependencies: &mut Dependencies,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self::Row<'source>, Vec<FieldError>), CatalogError> {
        let (prepared, notes) = crate::catalog_v3::mining_process_catalog::read_globals(
            tree, projection, admission, cancel,
        )?;
        Ok((prepared.into_definition(), notes))
    }
}

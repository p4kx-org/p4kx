// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Filter, Guid, Record, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ammo_presentation::AmmoPresentation;
use super::authored_ammo_inputs::AuthoredAmmoInputs;
pub(crate) use super::authored_damage::{AuthoredDamage, DamageChannels};
use super::authored_fields::Observed;
pub(crate) use super::authored_projectile::{AuthoredProjectile, ProjectileDamage};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::PublicNameRef;
use crate::entity_selection::{self, Selected};
use crate::export_diagnostic::ExportSourceRole;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "AmmoParams";

/// Why a physical ammunition row belongs to this catalog selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum AmmoSelection {
    /// The original Ammo Record was admitted independently.
    Selected,
    /// The original Ammo Record was reached by another selected owner.
    Dependency,
}

impl AmmoSelection {
    pub(crate) fn for_reached_record(record: &Record<'_>, filter: Option<&Filter>) -> Self {
        if matches!(
            entity_selection::select(record, filter),
            Ok(Some(Selected::Entity(Kind::AmmoDefinitions)))
        ) {
            Self::Selected
        } else {
            Self::Dependency
        }
    }

    pub(crate) const fn role(self) -> ExportSourceRole {
        match self {
            Self::Selected => ExportSourceRole::Selected,
            Self::Dependency => ExportSourceRole::Dependency,
        }
    }
}

/// One physical ammunition definition and its original configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct AmmoDefinition<'source> {
    selection: AmmoSelection,
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    inputs: AuthoredAmmoInputs<'source>,
    #[serde(flatten)]
    presentation: AmmoPresentation<'source>,
}

impl<'source> AmmoDefinition<'source> {
    /// Reports the physical row's reason for admission.
    #[must_use]
    pub const fn selection(&self) -> AmmoSelection {
        self.selection
    }

    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    pub(crate) const fn projectile(&self) -> &SourcedValue<AuthoredProjectile<'source>> {
        self.inputs.projectile()
    }

    /// Borrows the original speed and its source evidence.
    #[must_use]
    pub const fn speed(&self) -> &SourcedValue<ResourceFloat32> {
        self.inputs.speed()
    }

    /// Borrows the original lifetime and its source evidence.
    #[must_use]
    pub const fn lifetime(&self) -> &SourcedValue<ResourceFloat32> {
        self.inputs.lifetime()
    }

    /// Borrows the reference to the original localized short name.
    #[must_use]
    pub const fn display_name(&self) -> &SourcedValue<PublicNameRef> {
        self.presentation.display_name()
    }
}

pub(crate) struct PreparedAmmoDefinition<'source> {
    definition: AmmoDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> PreparedAmmoDefinition<'source> {
    pub(crate) const fn selection(&self) -> AmmoSelection {
        self.definition.selection()
    }

    pub(crate) fn into_definition(self) -> AmmoDefinition<'source> {
        self.definition
    }

    pub(super) const fn inputs(&self) -> &AuthoredAmmoInputs<'source> {
        &self.definition.inputs
    }
}

/// Projects typed inputs from the same admitted tree used by the legacy view.
pub(crate) fn read<'source>(
    projection: &CatalogProjection<'_, 'source>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    selection: AmmoSelection,
) -> Result<(PreparedAmmoDefinition<'source>, Vec<FieldError>), CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let mut notes = Vec::new();
    let observed = Observed::definition(&originals, input, &capture, &numbers);
    let inputs = AuthoredAmmoInputs::read(observed, &mut notes)?;
    let presentation = AmmoPresentation::read(observed, projection, &mut notes)?;
    Ok((
        PreparedAmmoDefinition {
            definition: AmmoDefinition {
                selection,
                source: CanonicalRecordHeader::read(tree, input.origins())?,
                inputs,
                presentation,
            },
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    ))
}

pub(crate) fn order(rows: &mut [PreparedAmmoDefinition<'_>]) {
    rows.sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedAmmoDefinition<'source>>,
) -> CatalogTable<AmmoDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter()
            .map(PreparedAmmoDefinition::into_definition)
            .collect(),
    )
}

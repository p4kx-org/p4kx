// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, DataForge, Filter, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::PhysicalComponents;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::referenced_definition_read::{self, Definitions, Selection};
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "lightning_behavior.rs"]
mod lightning_behavior;
#[path = "lightning_target.rs"]
mod lightning_target;

pub use lightning_behavior::{LightningBehavior, LightningEffect};
pub use lightning_target::{
    InternalPlanetLightning, LightningRange, LightningTargetMode, LightningTargetPayload,
};

pub(crate) const SOURCE_TYPE: &str = "ElectricalBehavior";
const LIGHTNING: &str = "lightning";

/// A physical electrical behaviour Record and its original lightning configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ElectricalBehavior<'source> {
    source: CanonicalRecordHeader<'source>,
    lightning: SourcedValue<LightningBehavior<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ElectricalBehavior<'source> {
    /// Borrows the original owner identity.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the original lightning configuration and its source evidence.
    #[must_use]
    pub const fn lightning(&self) -> &SourcedValue<LightningBehavior<'source>> {
        &self.lightning
    }

    /// Borrows original members outside the typed lightning configuration.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    pub(in crate::catalog_v3) fn read(
        _: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let lightning = fields.declared_class_to(
            LIGHTNING,
            lightning_behavior::SOURCE_TYPE,
            notes,
            LightningBehavior::read,
        )?;
        let additional_fields = fields.authored_members_except([(LIGHTNING, lightning.state())])?;
        Ok(Self {
            source,
            lightning,
            additional_fields,
        })
    }
}

fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Definitions<'source, ElectricalBehavior<'source>>, CatalogError> {
    referenced_definition_read::read(
        forge,
        input,
        &Selection {
            source_types: &[SOURCE_TYPE],
            linked: &BTreeSet::new(),
            filter,
            inspection: ConfigurationInspection::ElectricalBehavior,
        },
        cancel,
        ElectricalBehavior::read,
    )
}

impl<'source> PhysicalComponents<'_, '_, 'source> {
    pub(super) fn electrical_behaviors(
        &mut self,
    ) -> Result<CatalogTable<ElectricalBehavior<'source>>, CatalogError> {
        let definitions = read(
            self.forge,
            self.projection.original(),
            self.options.filter,
            self.options.cancel,
        )?;
        let rows = self.keep_definitions(definitions);
        Ok(CatalogTable::new(self.forge, &[SOURCE_TYPE], rows))
    }
}

#[cfg(test)]
#[path = "electrical_behavior_tests.rs"]
mod tests;

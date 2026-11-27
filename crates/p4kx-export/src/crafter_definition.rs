// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::{authored_fields::Observed, prepared_definition::PreparedDefinition};
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_row::CatalogRow;
use crate::catalog_table::CatalogTable;
use crate::record_fields::component_occurrence;
use crate::record_observation::RecordObservation;
use crate::scan::CRAFTER_STRUCT;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, Kind};

#[path = "crafter_station_fields.rs"]
mod station_fields;

pub use station_fields::{
    CrafterAttachment, CrafterAttachmentDefinition, CrafterCategoryRule, CrafterCategoryRuleKind,
    CrafterComponent, CrafterModifier, CrafterModifierKind, CrafterModifierType, CrafterQueue,
};

const ATTACHABLE: &str = "SAttachableComponentParams";

pub(crate) type PreparedCrafter<'source> = PreparedDefinition<'source, CrafterDefinition<'source>>;

/// One physical Entity that declares a crafting component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    component: SourcedValue<CrafterComponent<'source>>,
    attachment: SourcedValue<CrafterAttachment<'source>>,
}

impl<'source> CrafterDefinition<'source> {
    /// Borrows the original Entity Record and its separately sourced header facts.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the first readable original crafting component and its selection evidence.
    #[must_use]
    pub const fn component(&self) -> &SourcedValue<CrafterComponent<'source>> {
        &self.component
    }

    /// Borrows the original separately declared attachable component and its selection evidence.
    #[must_use]
    pub const fn attachment(&self) -> &SourcedValue<CrafterAttachment<'source>> {
        &self.attachment
    }

    pub(crate) fn selection_incomplete(&self) -> bool {
        !self.component.origin().source_set_complete()
            || !self.attachment.origin().source_set_complete()
    }

    fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        tree: &RecordTree<'source>,
    ) -> Result<Self, CatalogError> {
        let selected = |name| component_occurrence(tree, name).map(|(value, _)| value);
        let component =
            fields.selected_component(selected(CRAFTER_STRUCT), notes, CrafterComponent::read)?;
        let attachment =
            fields.selected_component(selected(ATTACHABLE), notes, CrafterAttachment::read)?;
        Ok(Self {
            source,
            component,
            attachment,
        })
    }
}

pub(crate) const fn candidate(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Crafters | Kind::Items | Kind::Ships | Kind::Commodities | Kind::MineableEntities
    )
}

pub(crate) fn prepare<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<Option<(PreparedCrafter<'source>, Vec<FieldError>)>, CatalogError> {
    if component_occurrence(tree, CRAFTER_STRUCT).is_none() {
        return Ok(None);
    }
    PreparedDefinition::read_original(tree, input, admission, cancel, |source, fields, notes| {
        CrafterDefinition::read(source, fields, notes, tree)
    })
    .map(Some)
}

pub(crate) fn order(rows: &mut [PreparedCrafter<'_>]) {
    PreparedDefinition::order(rows);
}

pub(crate) fn selection_incomplete(row: &PreparedCrafter<'_>) -> bool {
    row.definition().selection_incomplete()
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedCrafter<'source>>,
) -> CatalogTable<CrafterDefinition<'source>> {
    PreparedDefinition::table(forge, CRAFTER_STRUCT, rows)
}

pub(crate) fn catalog_rows<'rows>(
    rows: &'rows [PreparedCrafter<'_>],
) -> impl Iterator<Item = CatalogRow> + 'rows {
    PreparedDefinition::catalog_rows(rows, Kind::Crafters)
}

pub(crate) fn numbers<'rows>(
    rows: &'rows [PreparedCrafter<'_>],
) -> impl Iterator<Item = (Kind, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::Crafters, rows).map(|(kind, _, _, numbers)| (kind, numbers))
}

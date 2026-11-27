// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree};

use super::{
    BlueprintPoolDefinition, BlueprintPoolEntry, BlueprintPoolRead, PreparedBlueprintPool,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintPoolRead<'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let interpreted = crate::crafting_records::pool(tree);
    let weights = super::weights::read(&source, &originals, interpreted.total_weight())?;
    let definition = BlueprintPoolDefinition::project(source, fields, &mut notes)?;
    Ok(BlueprintPoolRead {
        prepared: PreparedBlueprintPool {
            definition,
            weights,
            numbers: NumberCapture::deduplicate(numbers.into_inner())?,
            physical_guid: tree.guid(),
            physical_path: tree.record_path(),
            physical_index: tree.record_index(),
        },
        interpreted,
        notes,
        complete: crate::source_value_catalog::retained_values_complete(&originals),
    })
}

impl<'source> BlueprintPoolDefinition<'source> {
    fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let blueprint_rewards =
            fields.declared_owned_array("blueprintRewards", notes, BlueprintPoolEntry::read)?;
        let additional_fields =
            fields.authored_members_except([("blueprintRewards", blueprint_rewards.state())])?;
        Ok(Self {
            source,
            blueprint_rewards,
            additional_fields,
        })
    }
}

impl<'source> BlueprintPoolEntry<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let blueprint_record = fields.record_link("blueprintRecord", notes)?;
        let weight = fields.authored_number("weight", notes)?;
        let additional_fields = fields.authored_members_except([
            ("blueprintRecord", blueprint_record.state()),
            ("weight", weight.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            blueprint_record,
            weight,
            additional_fields,
        })
    }
}

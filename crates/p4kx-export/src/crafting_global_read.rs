// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataType, RecordTree};

use super::{
    CraftingBlueprintSelection, CraftingGlobalConfiguration, CraftingGlobalRead,
    PreparedCraftingGlobal,
};
use crate::CatalogError;
use crate::blueprint_rule_inputs::BlueprintRules;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::authored_fields::Observed;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

const DEFAULTS: &str = "defaultBlueprintSelection";
const RESOURCES: &str = "dismantleBlacklistResources";
const ITEMS: &str = "dismantleBlacklistEntityClasses";
const BLUEPRINTS: &str = "blueprintRecords";

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<CraftingGlobalRead<'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::configuration(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let configuration = CraftingGlobalConfiguration::project(source, fields, &mut notes)?;
    let rules = BlueprintRules::read_globals_fields(tree, &originals)?;
    let complete = crate::source_value_catalog::retained_values_complete(&originals);
    Ok(CraftingGlobalRead {
        prepared: PreparedCraftingGlobal {
            configuration,
            numbers: NumberCapture::deduplicate(numbers.into_inner())?,
        },
        rules,
        notes,
        complete,
    })
}

impl<'source> CraftingGlobalConfiguration<'source> {
    fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let default_blueprint_selection =
            if fields.storage_type(DEFAULTS)? == Some(DataType::StrongPointer) {
                fields.declared_pointer(DEFAULTS, notes, CraftingBlueprintSelection::read)?
            } else {
                fields.declared_class(DEFAULTS, notes, CraftingBlueprintSelection::read)?
            };
        let dismantle_blacklist_resources = fields.record_link_array(RESOURCES, notes)?;
        let dismantle_blacklist_entity_classes = fields.record_link_array(ITEMS, notes)?;
        let additional_fields = fields.authored_members_except([
            (DEFAULTS, default_blueprint_selection.state()),
            (RESOURCES, dismantle_blacklist_resources.state()),
            (ITEMS, dismantle_blacklist_entity_classes.state()),
        ])?;
        Ok(Self {
            source,
            default_blueprint_selection,
            dismantle_blacklist_resources,
            dismantle_blacklist_entity_classes,
            additional_fields,
        })
    }
}

impl<'source> CraftingBlueprintSelection<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let blueprint_records = source.record_link_array(BLUEPRINTS, notes)?;
        let additional_fields =
            source.authored_members_except([(BLUEPRINTS, blueprint_records.state())])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            blueprint_records,
            additional_fields,
        })
    }
}

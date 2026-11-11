// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourcedValue};

use crate::CatalogError;
use crate::blueprint_choice_inputs::{self, BlueprintChoice};
use crate::blueprint_definition::{BlueprintDefinition, PhysicalBlueprintRef};
use crate::blueprint_field_source::BlueprintField;
use crate::blueprint_presentation::BlueprintIngredient;
use crate::blueprint_tier_contract::BlueprintTier;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_text_contract::GameTextRef;
use crate::crafting_records;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceValueRef;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) struct PreparedBlueprint<'source> {
    pub(crate) record_index: u32,
    pub(crate) definition: BlueprintDefinition<'source>,
    pub(crate) tiers: Vec<BlueprintTier>,
    pub(crate) unknown_range_types: Vec<&'source str>,
    pub(crate) first_tier: Option<SourceValueRef>,
    pub(crate) ingredients: Vec<BlueprintIngredient>,
    pub(crate) category: BlueprintChoice<CatalogGuid>,
    pub(crate) produced_item: BlueprintChoice<CatalogGuid>,
    pub(crate) name: SourcedValue<GameTextRef>,
    pub(crate) resources: crate::blueprint_backfill_source::BlueprintResources,
}

pub(crate) struct BlueprintRead<'source> {
    pub(crate) value: PreparedBlueprint<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Interprets one physical admitted Blueprint and retains its complete original source arena.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintRead<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let numbers = NumberCapture::default();
    let indexed = crate::source_value_catalog::read_indexed_arena(&scope, cancel, |site| {
        numbers.retain_blueprint(site);
    })?;
    let fields = &indexed.facts;
    let capture = TemplateFieldCapture::new(fields, input, admission)?;
    let source = crafting_records::blueprint_source(tree);
    let mut unknown = Vec::new();
    let interpreted = crate::crafting_tier::tiers(source, &mut unknown, cancel)?;
    let owner = PhysicalBlueprintRef {
        input_ref: fields.input_ref().clone(),
        record_index: tree.record_index(),
    };
    let mut notes = Vec::new();
    let tiers = interpreted
        .iter()
        .map(|tier| {
            crate::blueprint_tier_read::read(
                tier,
                owner.clone(),
                projection,
                fields,
                &capture,
                &mut notes,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let resources = crate::blueprint_backfill_source::read(&interpreted, fields)?;
    let first_tier = crate::blueprint_ingredient_read::first_ref(interpreted.first(), fields)?;
    let ingredients = crate::blueprint_ingredient_read::read(interpreted.first(), fields)?;
    let AuthoredBlueprint {
        category,
        produced_item,
        name,
    } = authored(fields, tree, projection)?;
    Ok(BlueprintRead {
        value: PreparedBlueprint {
            record_index: tree.record_index(),
            definition: BlueprintDefinition {
                physical_guid: CatalogGuid(tree.guid()),
                physical_path: tree.record_path(),
                physical_index: tree.record_index(),
                input_ref: owner.input_ref,
                header: CanonicalRecordHeader::read(tree, projection.origins())?,
                source: indexed.source,
                numbers: numbers.finish(),
            },
            tiers,
            unknown_range_types: unknown,
            first_tier,
            ingredients,
            category,
            produced_item,
            name,
            resources,
        },
        notes,
    })
}

struct AuthoredBlueprint {
    category: BlueprintChoice<CatalogGuid>,
    produced_item: BlueprintChoice<CatalogGuid>,
    name: SourcedValue<GameTextRef>,
}

fn authored<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<AuthoredBlueprint, CatalogError> {
    let authored = BlueprintField::read(fields, tree.root(), "blueprint")?;
    let category = blueprint_choice_inputs::reference(&authored.child("category")?)?;
    let produced_item = blueprint_choice_inputs::reference(
        &authored
            .child("processSpecificData")?
            .child("entityClass")?,
    )?;
    let name = crate::blueprint_tier_read::text(&authored.child("blueprintName")?, projection)?;
    Ok(AuthoredBlueprint {
        category,
        produced_item,
        name,
    })
}

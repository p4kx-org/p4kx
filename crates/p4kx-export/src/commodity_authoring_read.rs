// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    RecordTree, SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue,
};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::commodity_authoring_contract::{
    CommodityAuthoring, CommodityContainerLink, CommodityDisplay, CommodityPurchasable,
};
use crate::commodity_authoring_inputs;
use crate::commodity_binding_read::{self, BindingArray, BindingAt, BindingFacts, BindingRead};
use crate::commodity_inputs::CommodityInputs;
use crate::commodity_text_input::CommodityTextInput;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

pub(crate) struct CommodityAuthoringRead<'source> {
    pub(crate) value: CommodityAuthoring<'source>,
    pub(crate) own_name: CommodityTextInput<'source>,
    pub(crate) own_description: CommodityTextInput<'source>,
    pub(crate) display_type: CommodityTextInput<'source>,
    pub(crate) inputs: Vec<SourceUse>,
    pub(crate) complete: bool,
    pub(crate) crafter_inputs: Vec<SourceUse>,
    pub(crate) crafter_complete: bool,
}

/// Reads commodity-owned declarations and the first known presentation candidates.
pub(crate) fn entity<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CommodityAuthoringRead<'source>, CatalogError> {
    let origins = projection.origins();
    let context = origins.record(tree.record_index());
    let components = BindingArray::new(tree, origins, "Components", notes)?;
    let (crafter_inputs, crafter_complete) =
        components.first_type_inputs(tree.record_index(), origins, "CrafterComponentParams")?;
    let static_data = BindingArray::new(tree, origins, "StaticEntityClassData", notes)?;
    let purchases = commodity_binding_read::collect(
        tree,
        origins,
        &components,
        "SCItemPurchasableParams",
        notes,
        |source, at, notes| {
            purchasable(source, at, tree.record_index(), projection, &context, notes)
        },
    )?;
    let displays = commodity_binding_read::collect(
        tree,
        origins,
        &static_data,
        "EntityUIDisplayParams",
        notes,
        |source, at, notes| display(source, at, tree.record_index(), projection, &context, notes),
    )?;
    let containers = commodity_binding_read::collect(
        tree,
        origins,
        &components,
        "ResourceContainer",
        notes,
        |source, at, _| {
            Ok((
                container_link(source, at, tree.record_index(), projection, &context)?,
                (),
            ))
        },
    )?;
    Ok(AuthoredBindings {
        purchases,
        displays,
        containers,
        crafter_inputs,
        crafter_complete,
    }
    .finish(origins))
}

struct AuthoredBindings<'source> {
    purchases: BindingRead<'source, CommodityPurchasable<'source>, PurchasableInputs<'source>>,
    displays: BindingRead<'source, CommodityDisplay<'source>, CommodityTextInput<'source>>,
    containers: BindingRead<'source, CommodityContainerLink<'source>, ()>,
    crafter_inputs: Vec<SourceUse>,
    crafter_complete: bool,
}

impl<'source> AuthoredBindings<'source> {
    #[must_use]
    fn finish(self, origins: &CatalogOrigins) -> CommodityAuthoringRead<'source> {
        let Self {
            purchases,
            displays,
            containers,
            crafter_inputs,
            crafter_complete,
        } = self;
        let candidates = purchases.first.unwrap_or_else(|| PurchasableInputs {
            own_name: purchases.parent.clone(),
            display_type: purchases.parent.clone(),
        });
        let own_description = displays.first.unwrap_or_else(|| displays.parent.clone());
        let mut aggregate = CommodityInputs::new(origins);
        aggregate.extend(purchases.inputs, purchases.complete);
        aggregate.extend(displays.inputs, displays.complete);
        aggregate.extend(containers.inputs, containers.complete);
        let (inputs, complete) = aggregate.into_evidence();
        CommodityAuthoringRead {
            value: CommodityAuthoring {
                purchasable_parameters: purchases.value,
                display_parameters: displays.value,
                resource_containers: containers.value,
            },
            own_name: candidates.own_name.with_selectors(
                &purchases.parent.selectors,
                purchases.parent.selectors_complete,
            ),
            own_description: own_description.with_selectors(
                &displays.parent.selectors,
                displays.parent.selectors_complete,
            ),
            display_type: candidates.display_type.with_selectors(
                &purchases.parent.selectors,
                purchases.parent.selectors_complete,
            ),
            inputs,
            complete,
            crafter_inputs,
            crafter_complete,
        }
    }
}

struct PurchasableInputs<'source> {
    own_name: CommodityTextInput<'source>,
    display_type: CommodityTextInput<'source>,
}

fn purchasable<'source>(
    source: &StructValue<'source>,
    at: &BindingAt<'_>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<
    (
        BindingFacts<CommodityPurchasable<'source>>,
        PurchasableInputs<'source>,
    ),
    CatalogError,
> {
    let fields = CatalogFields::new(source, at.path, context);
    let name_path = format!("{}.displayName", at.path);
    let type_path = format!("{}.displayType", at.path);
    let name = fields.locale("displayName", notes)?;
    let display_type = fields.locale("displayType", notes)?;
    let name_input = CommodityTextInput::new(name.clone(), record_index, name_path.clone());
    let type_input = CommodityTextInput::new(display_type.clone(), record_index, type_path.clone());
    let mut inputs = binding_inputs(projection, record_index, at)?;
    commodity_authoring_inputs::field(&mut inputs, record_index, &name_path, &name)?;
    commodity_authoring_inputs::field(&mut inputs, record_index, &type_path, &display_type)?;
    let value = CommodityPurchasable {
        source_position: position(context, at)?,
        source_type: fields.source_type()?,
        display_name: projection.name(name, record_index, &name_path)?,
        display_type: projection.description(display_type, record_index, &type_path)?,
    };
    Ok((
        BindingFacts::new(value, inputs),
        PurchasableInputs {
            own_name: name_input,
            display_type: type_input,
        },
    ))
}

fn display<'source>(
    source: &StructValue<'source>,
    at: &BindingAt<'_>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<
    (
        BindingFacts<CommodityDisplay<'source>>,
        CommodityTextInput<'source>,
    ),
    CatalogError,
> {
    let fields = CatalogFields::new(source, at.path, context);
    let field = format!("{}.displayDescription", at.path);
    let key = fields.locale("displayDescription", notes)?;
    let input = CommodityTextInput::new(key.clone(), record_index, field.clone());
    let mut inputs = binding_inputs(projection, record_index, at)?;
    commodity_authoring_inputs::field(&mut inputs, record_index, &field, &key)?;
    let value = CommodityDisplay {
        source_position: position(context, at)?,
        source_type: fields.source_type()?,
        display_description: projection.description(key, record_index, &field)?,
    };
    Ok((BindingFacts::new(value, inputs), input))
}

fn container_link<'source>(
    source: &StructValue<'source>,
    at: &BindingAt<'_>,
    record_index: u32,
    projection: &CatalogProjection<'_, 'source>,
    context: &CatalogRecordContext<'_>,
) -> Result<BindingFacts<CommodityContainerLink<'source>>, CatalogError> {
    let fields = CatalogFields::new(source, at.path, context);
    let mut inputs = binding_inputs(projection, record_index, at)?;
    inputs.header_input(
        record_index,
        RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
        SourceUseRole::Selector,
    )?;
    Ok(BindingFacts::new(
        CommodityContainerLink {
            owner_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(record_index),
            )?,
            source_position: position(context, at)?,
            source_type: fields.source_type()?,
        },
        inputs,
    ))
}

fn binding_inputs<'context>(
    projection: &'context CatalogProjection<'_, '_>,
    record_index: u32,
    at: &BindingAt<'_>,
) -> Result<CommodityInputs<'context>, CatalogError> {
    let mut inputs = CommodityInputs::new(projection.origins());
    inputs.at(record_index, at.path, SourceUseRole::Selector)?;
    inputs.at(
        record_index,
        &format!("{}.$type", at.path),
        SourceUseRole::Selector,
    )?;
    Ok(inputs)
}

fn position(
    context: &CatalogRecordContext<'_>,
    at: &BindingAt<'_>,
) -> Result<SourcedValue<u32>, CatalogError> {
    let position = u32::try_from(at.position)
        .map_err(|_| CatalogError::Accounting(ExportAccountingError::CounterOverflow))?;
    context.selector(at.path, SourceValue::Present(position))
}

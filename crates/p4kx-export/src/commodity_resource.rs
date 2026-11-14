// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{ExampleBodyReceipt, OriginalFlowFields};

use crate::definition_reference::DefinitionReference;
use crate::record_observation::RecordObservation;
use crate::source_fields::{self, FieldError, Fields};

/// The first authored resource link and its original indexed field identity.
pub(crate) struct CommodityResource<'tree, 'source> {
    pub(crate) reference: DefinitionReference,
    pub(crate) field: String,
    entry: &'tree StructValue<'source>,
    value: &'tree Value<'source>,
}

impl<'source> CommodityResource<'_, 'source> {
    pub(crate) fn failure_body(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        let scope = input.record_scope(admission.record(), tree)?;
        let fields = OriginalFlowFields::read(&scope, cancel)?;
        let fact = fields.fact(self.value)?;
        let ancestors = fields.ancestors(fact)?;
        fields.body_capture(input, admission)?.reference_fact(
            self.entry,
            fact,
            &ancestors,
            &self.field,
        )
    }
}

/// The established first-container choice and every selector actually used to reach it.
pub(crate) struct CommodityResourceSelection<'tree, 'source> {
    pub(crate) link: Option<CommodityResource<'tree, 'source>>,
    pub(crate) selectors: Vec<String>,
}

/// Returns the first non-null resource link without replacing an unresolved first target.
pub(crate) fn first<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Result<CommodityResourceSelection<'tree, 'source>, FieldError> {
    let mut selected = CommodityResourceSelection {
        link: None,
        selectors: vec!["Components".to_owned()],
    };
    let root = Fields::new(tree.root(), "");
    let Some(components) = root.array("Components")? else {
        return Ok(selected);
    };
    for (index, value) in components.iter().enumerate() {
        let path = format!("Components[{index}]");
        let source = source_fields::entry(value, tree.type_name(), &path)?;
        selected.selectors.push(if source.is_some() {
            format!("{path}.$type")
        } else {
            path.clone()
        });
        if let Some(source) = source.filter(|source| source.type_name() == "ResourceContainer") {
            selected.link = composition(source, &path, &mut selected.selectors)?;
            return Ok(selected);
        }
    }
    Ok(selected)
}

fn composition<'tree, 'source>(
    source: &'tree StructValue<'source>,
    path: &str,
    selectors: &mut Vec<String>,
) -> Result<Option<CommodityResource<'tree, 'source>>, FieldError> {
    let fields = Fields::new(source, path);
    selectors.push(format!("{path}.defaultComposition"));
    let Some(values) = fields.array("defaultComposition")? else {
        return Ok(None);
    };
    for (index, value) in values.iter().enumerate() {
        let entry_path = format!("{path}.defaultComposition[{index}]");
        let entry = source_fields::entry(value, source.type_name(), &entry_path)?;
        selectors.push(if entry.is_some() {
            format!("{entry_path}.$type")
        } else {
            entry_path.clone()
        });
        let Some(entry) = entry else {
            continue;
        };
        selectors.push(format!("{entry_path}.entry"));
        if let Some(slot) = Fields::new(entry, &entry_path).reference_slot("entry")? {
            return Ok(Some(CommodityResource {
                reference: slot.reference.into_owned(),
                field: format!("{entry_path}.entry"),
                entry,
                value: slot.value,
            }));
        }
    }
    Ok(None)
}

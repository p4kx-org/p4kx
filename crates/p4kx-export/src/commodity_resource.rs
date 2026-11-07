// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue};

use crate::definition_reference::DefinitionReference;
use crate::source_fields::{self, FieldError, Fields};

/// The first authored resource link and its original indexed field identity.
pub(crate) struct CommodityResource {
    pub(crate) reference: DefinitionReference,
    pub(crate) field: String,
    pub(crate) source_type: String,
}

/// Returns the first non-null resource link with its canonical physical target.
pub(crate) fn first(tree: &RecordTree<'_>) -> Result<Option<CommodityResource>, FieldError> {
    let root = Fields::new(tree.root(), "");
    let Some(components) = root.array("Components")? else {
        return Ok(None);
    };
    for (index, value) in components.iter().enumerate() {
        let path = format!("Components[{index}]");
        let source = source_fields::entry(value, tree.type_name(), &path)?;
        if let Some(source) = source.filter(|source| source.type_name() == "ResourceContainer") {
            return composition(source, &path);
        }
    }
    Ok(None)
}

fn composition(
    source: &StructValue<'_>,
    path: &str,
) -> Result<Option<CommodityResource>, FieldError> {
    let fields = Fields::new(source, path);
    let Some(values) = fields.array("defaultComposition")? else {
        return Ok(None);
    };
    for (index, value) in values.iter().enumerate() {
        let entry_path = format!("{path}.defaultComposition[{index}]");
        let entry = source_fields::entry(value, source.type_name(), &entry_path)?;
        let Some(entry) = entry else { continue };
        if let Some(reference) = Fields::new(entry, &entry_path).reference("entry")? {
            return Ok(Some(CommodityResource {
                reference,
                field: format!("{entry_path}.entry"),
                source_type: entry.type_name().to_owned(),
            }));
        }
    }
    Ok(None)
}

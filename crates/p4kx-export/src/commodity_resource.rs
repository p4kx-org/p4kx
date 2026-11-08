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

/// The established first-container choice and every selector actually used to reach it.
pub(crate) struct CommodityResourceSelection {
    pub(crate) link: Option<CommodityResource>,
    pub(crate) selectors: Vec<String>,
}

/// Returns the first non-null resource link without replacing an unresolved first target.
pub(crate) fn first(tree: &RecordTree<'_>) -> Result<CommodityResourceSelection, FieldError> {
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

fn composition(
    source: &StructValue<'_>,
    path: &str,
    selectors: &mut Vec<String>,
) -> Result<Option<CommodityResource>, FieldError> {
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

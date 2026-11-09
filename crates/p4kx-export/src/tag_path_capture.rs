// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Guid, SourceLocation, SourceUse, SourceUseRole, SourceValue, SourceValueState,
};

use super::contract::{TagPath, TagPathParent};
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::export_ledger::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;

struct NodeInputs {
    record_index: u32,
    guid: Guid,
    construction: Vec<SourceUse>,
    selection: Vec<SourceUse>,
    complete: bool,
}

#[derive(Default)]
pub(super) struct PathCapture {
    nodes: Vec<NodeInputs>,
    pub(super) rows: Vec<TagPath>,
}

pub(super) struct ChosenPath<'a> {
    pub(super) record_index: u32,
    pub(super) guid: Guid,
    pub(super) position: u32,
    pub(super) parent_position: Option<u32>,
    pub(super) incoming: Option<(u32, &'static str, usize)>,
    pub(super) path: &'a str,
    pub(super) depth: u16,
    pub(super) listed: bool,
    pub(super) name_state: SourceValueState,
}

impl PathCapture {
    pub(super) fn chosen(
        &mut self,
        origins: &CatalogOrigins,
        chosen: &ChosenPath<'_>,
    ) -> Result<(), CatalogError> {
        if usize::try_from(chosen.position).ok() != Some(self.nodes.len()) {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        }
        let parent = chosen
            .parent_position
            .map(|position| {
                usize::try_from(position)
                    .ok()
                    .and_then(|position| self.nodes.get(position))
                    .ok_or(ExportAccountingError::ConflictingTagTrace)
            })
            .transpose()?;
        let inputs = NodeInputs::derive(origins, chosen, parent)?;
        self.rows.push(row(origins, chosen, parent, &inputs)?);
        self.nodes.push(inputs);
        Ok(())
    }
}

impl NodeInputs {
    fn derive(
        origins: &CatalogOrigins,
        chosen: &ChosenPath<'_>,
        parent: Option<&Self>,
    ) -> Result<Self, CatalogError> {
        let mut construction = parent.map_or_else(Vec::new, |parent| parent.construction.clone());
        let mut selection = parent.map_or_else(Vec::new, |parent| parent.selection.clone());
        let complete = parent.is_none_or(|parent| parent.complete)
            && chosen.name_state != SourceValueState::Unreadable;
        let mut qualification = header_inputs(origins, chosen.record_index)?;
        if let Some((owner, field, ordinal)) = chosen.incoming {
            qualification.push(observe(
                origins,
                owner,
                &format!("{field}[{ordinal}]"),
                SourceUseRole::Selector,
            )?);
            if parent.is_none() {
                qualification.extend(header_inputs(origins, owner)?);
            }
        }
        let selector = if chosen.name_state == SourceValueState::Unreadable {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        selection.extend(qualification.iter().cloned());
        selection.push(observe(origins, chosen.record_index, "tagName", selector)?);
        construction.extend(qualification);
        let role = if chosen.name_state == SourceValueState::Present {
            SourceUseRole::Conversion
        } else {
            selector
        };
        construction.push(observe(origins, chosen.record_index, "tagName", role)?);
        Ok(Self {
            record_index: chosen.record_index,
            guid: chosen.guid,
            construction,
            selection,
            complete,
        })
    }
}

fn row(
    origins: &CatalogOrigins,
    chosen: &ChosenPath<'_>,
    parent: Option<&NodeInputs>,
    inputs: &NodeInputs,
) -> Result<TagPath, CatalogError> {
    let context = origins.record(chosen.record_index);
    let parent_value = match parent {
        None => TagPathParent::Root,
        Some(parent) => {
            let parent_context = origins.record(parent.record_index);
            TagPathParent::Tag {
                guid: parent_context.header_value(
                    RecordHeaderField::Guid,
                    SourceValue::Present(CatalogGuid(parent.guid)),
                )?,
                record_index: parent_context.header_selector(
                    RecordHeaderField::RecordIndex,
                    SourceValue::Present(parent.record_index),
                )?,
            }
        }
    };
    let mut parent_inputs = inputs.selection.clone();
    if let Some(parent) = parent {
        parent_inputs.push(
            origins
                .record(parent.record_index)
                .observe_header(RecordHeaderField::Guid, SourceUseRole::Payload)?,
        );
    }
    Ok(TagPath {
        guid: context.header_value(
            RecordHeaderField::Guid,
            SourceValue::Present(CatalogGuid(chosen.guid)),
        )?,
        record_index: context.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(chosen.record_index),
        )?,
        path: origins.uses.value(
            SourceValue::Present(chosen.path.to_owned()),
            &inputs.construction,
            inputs.complete,
        ),
        depth: origins.uses.value(
            SourceValue::Present(u32::from(chosen.depth)),
            &inputs.selection,
            inputs.complete,
        ),
        listed: origins.uses.value(
            SourceValue::Present(chosen.listed),
            &inputs.selection,
            inputs.complete,
        ),
        parent: origins.uses.value(
            SourceValue::Present(parent_value),
            &parent_inputs,
            inputs.complete,
        ),
    })
}

fn header_inputs(
    origins: &CatalogOrigins,
    record_index: u32,
) -> Result<Vec<SourceUse>, CatalogError> {
    let context = origins.record(record_index);
    [
        RecordHeaderField::Guid,
        RecordHeaderField::SourceType,
        RecordHeaderField::RecordIndex,
    ]
    .into_iter()
    .map(|field| context.observe_header(field, SourceUseRole::Selector))
    .collect()
}

fn observe(
    origins: &CatalogOrigins,
    record_index: u32,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse, CatalogError> {
    Ok(origins.dataforge.at(
        SourceLocation::DataForge {
            record_index,
            field: field.to_owned(),
        },
        role,
    )?)
}

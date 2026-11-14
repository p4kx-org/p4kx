// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::iter;
use std::sync::Arc;

use p4kx_extraction::{
    CancelToken, Guid, Record, SourceLocation, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourceValueState, SourcedValue, TagPathEdgeKind, TagPathIncoming, TagTree,
};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::export_ledger::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;

#[derive(Debug)]
pub(super) struct ParentInputs {
    pub(super) position: u32,
    pub(super) payload: SourceUse,
}

#[derive(Debug)]
pub(super) struct NodeInputs {
    pub(super) record_index: u32,
    pub(super) guid: Guid,
    pub(super) guid_cell: SourcedValue<CatalogGuid>,
    pub(super) record_index_cell: SourcedValue<u32>,
    pub(super) parent: Option<ParentInputs>,
    pub(super) depth: u32,
    pub(super) listed: bool,
    pub(super) path_complete: bool,
    qualification: Vec<SourceUse>,
    constructed_name: SourceUse,
}

#[derive(Debug)]
pub(super) struct PathReceipts {
    pub(super) tree: Arc<TagTree>,
    pub(super) uses: SourceUses,
    nodes: Vec<NodeInputs>,
}

#[derive(Clone, Copy)]
pub(super) enum PathUse {
    Topology,
    Construction,
}

impl PathReceipts {
    pub(super) const fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn get(&self, position: usize) -> Option<&NodeInputs> {
        self.nodes.get(position)
    }

    pub(super) fn lineage(&self, position: usize, kind: PathUse) -> Vec<SourceUse> {
        iter::successors(self.nodes.get(position), |node| {
            self.nodes.get(node.parent.as_ref()?.position as usize)
        })
        .flat_map(|node| node.local_uses(kind))
        .cloned()
        .collect()
    }

    pub(super) fn append_path_union(
        &self,
        starts: impl IntoIterator<Item = usize>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<(), ExportAccountingError> {
        let mut visited = HashSet::new();
        for start in starts {
            let mut next = Some(start);
            while let Some(position) = next {
                if !visited.insert(position) {
                    break;
                }
                let node = self
                    .get(position)
                    .ok_or(ExportAccountingError::ConflictingTagTrace)?;
                uses.extend(node.local_uses(PathUse::Construction).cloned());
                next = node.parent.as_ref().map(|parent| parent.position as usize);
            }
        }
        Ok(())
    }
}

impl NodeInputs {
    fn local_uses(&self, kind: PathUse) -> impl Iterator<Item = &SourceUse> {
        let name = match kind {
            PathUse::Topology => None,
            PathUse::Construction => Some(&self.constructed_name),
        };
        self.qualification.iter().chain(name)
    }
}

#[derive(Default)]
pub(super) struct PathCapture {
    nodes: Vec<NodeInputs>,
}

pub(super) struct ChosenPath<'record, 'source> {
    pub(super) record: &'record Record<'source>,
    pub(super) position: u32,
    pub(super) parent_position: Option<u32>,
    pub(super) incoming: Option<TagPathIncoming<'record, 'source>>,
    pub(super) depth: u32,
    pub(super) listed: bool,
    pub(super) name: NameEvidence,
}

pub(super) enum NameEvidence {
    Absent(SourceUse),
    Declared(SourceValueState),
}

impl NameEvidence {
    fn construction(
        &self,
        origins: &CatalogOrigins,
        record_index: u32,
    ) -> Result<(SourceUse, bool), CatalogError> {
        let role = match self {
            Self::Absent(survey) => return Ok((survey.clone(), true)),
            Self::Declared(SourceValueState::Absent) => {
                return Err(ExportAccountingError::ConflictingTagTrace.into());
            }
            Self::Declared(SourceValueState::Present) => SourceUseRole::Conversion,
            Self::Declared(SourceValueState::Unreadable) => SourceUseRole::Attempted,
            Self::Declared(_) => SourceUseRole::Selector,
        };
        let name = observe(origins, record_index, "tagName", role)?;
        Ok((name, role != SourceUseRole::Attempted))
    }
}

impl PathCapture {
    pub(super) fn chosen(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        chosen: &ChosenPath<'_, '_>,
    ) -> Result<(), CatalogError> {
        if usize::try_from(chosen.position).ok() != Some(self.nodes.len()) {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        }
        let parent = chosen
            .parent_position
            .map(|position| {
                self.nodes
                    .get(position as usize)
                    .ok_or(ExportAccountingError::ConflictingTagTrace)
            })
            .transpose()?;
        let node = NodeInputs::derive(input, chosen, parent)?;
        self.nodes.push(node);
        Ok(())
    }

    pub(super) fn finish(
        self,
        tree: Arc<TagTree>,
        uses: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<Arc<PathReceipts>, CatalogError> {
        if tree.len() != self.nodes.len() {
            return Err(ExportAccountingError::ConflictingTagTrace.into());
        }
        for (tag, node) in tree.iter().zip(&self.nodes) {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            if tag.guid() != node.guid
                || tag.parent().map(|parent| parent.position())
                    != node.parent.as_ref().map(|parent| parent.position as usize)
                || tag.depth() != node.depth as usize
                || tag.is_listed() != node.listed
            {
                return Err(ExportAccountingError::ConflictingTagTrace.into());
            }
        }
        Ok(Arc::new(PathReceipts {
            tree,
            uses: uses.clone(),
            nodes: self.nodes,
        }))
    }
}

impl NodeInputs {
    pub(super) const fn name_input(&self) -> &SourceUse {
        &self.constructed_name
    }

    fn derive(
        input: &OriginalCatalogInput<'_, '_>,
        chosen: &ChosenPath<'_, '_>,
        parent: Option<&Self>,
    ) -> Result<Self, CatalogError> {
        let context = input.record_context(chosen.record)?;
        check_edge(input, chosen, parent)?;
        let origins = input.origins();
        let record_index = chosen.record.index();
        let guid = chosen.record.guid();
        let mut qualification = header_inputs(origins, record_index)?;
        if let Some(edge) = chosen.incoming {
            let owner = edge.record().index();
            qualification.push(observe(
                origins,
                owner,
                &format!("{}[{}]", edge.kind().field(), edge.ordinal()),
                SourceUseRole::Selector,
            )?);
            if parent.is_none() {
                qualification.extend(header_inputs(origins, owner)?);
            }
        }
        let (constructed_name, readable) = chosen.name.construction(origins, record_index)?;
        let path_complete = parent.is_none_or(|parent| parent.path_complete) && readable;
        Ok(Self {
            record_index,
            guid,
            guid_cell: context.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(guid)),
            )?,
            record_index_cell: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(record_index),
            )?,
            parent: parent
                .zip(chosen.parent_position)
                .map(|(parent, position)| parent_inputs(origins, parent, position))
                .transpose()?,
            depth: chosen.depth,
            listed: chosen.listed,
            path_complete,
            qualification,
            constructed_name,
        })
    }
}

fn check_edge(
    input: &OriginalCatalogInput<'_, '_>,
    chosen: &ChosenPath<'_, '_>,
    parent: Option<&NodeInputs>,
) -> Result<(), CatalogError> {
    let Some(edge) = chosen.incoming else {
        return if parent.is_none() && !chosen.listed && chosen.depth == 0 {
            Ok(())
        } else {
            Err(ExportAccountingError::ConflictingTagTrace.into())
        };
    };
    input.record_context(edge.record())?;
    let valid = match (parent, edge.kind()) {
        (None, TagPathEdgeKind::Root) => chosen.listed && chosen.depth == 0,
        (Some(parent), TagPathEdgeKind::Child) => {
            parent.record_index == edge.record().index()
                && parent.guid == edge.record().guid()
                && parent.listed == chosen.listed
                && parent.depth.checked_add(1) == Some(chosen.depth)
        }
        _ => false,
    };
    if !valid {
        return Err(ExportAccountingError::ConflictingTagTrace.into());
    }
    Ok(())
}

fn parent_inputs(
    origins: &CatalogOrigins,
    parent: &NodeInputs,
    position: u32,
) -> Result<ParentInputs, CatalogError> {
    let context = origins.record(parent.record_index);
    Ok(ParentInputs {
        position,
        payload: context.observe_header(RecordHeaderField::Guid, SourceUseRole::Payload)?,
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

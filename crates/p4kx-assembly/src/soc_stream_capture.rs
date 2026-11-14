// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::PathIndex;
use p4kx_extraction::cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_model::{SocContainerDeclaration, SocContainerDescriptor, SocSourceValue};

use super::soc_descriptor_member::SocMemberSelection;
use super::{SocCaptureError, SocGeometryReference};
use crate::interior::plain_container_path;

/// The original name selector and positions of an authored container member.
#[derive(Debug)]
pub struct SocMemberSelector {
    /// Section slot and declaration slot in their immediate XML owners.
    pub ordinal: (usize, usize),
    /// Original declaration XML coordinate.
    pub position: SourceElementPosition,
    /// Original name interpretation.
    pub name: SocSourceValue<String>,
    /// Every original name attribute's coordinate, including repeats.
    pub name_attributes: Vec<SourceAttributePosition>,
}

impl SocMemberSelector {
    pub(super) fn from_source(declaration: SocContainerDeclaration<'_>) -> Self {
        let name = match declaration.name() {
            SocSourceValue::Present(text) => SocSourceValue::Present(text.to_owned()),
            SocSourceValue::Absent => SocSourceValue::Absent,
            _ => SocSourceValue::Invalid,
        };
        Self {
            ordinal: declaration.ordinal(),
            position: declaration.element().position(),
            name,
            name_attributes: declaration
                .name_attributes()
                .map(|attr| attr.position().clone())
                .collect(),
        }
    }

    pub(super) fn path(&self) -> Option<&str> {
        match &self.name {
            SocSourceValue::Present(path) if !path.is_empty() && plain_container_path(path) => {
                Some(path)
            }
            _ => None,
        }
    }
}

/// An original stream declaration and its selected physical member, if any.
#[derive(Debug)]
pub struct SocStreamInput {
    /// The original descriptor occurrence selecting this stream.
    pub selector: SocMemberSelector,
    /// Original parent stream coordinate when nested in the descriptor.
    pub parent: Option<SourceElementPosition>,
    /// The admitted physical selection, absent when the selector is unusable.
    pub member: Option<SocMemberSelection>,
    /// Directory lookup of the original model slots in this logical stream occurrence.
    pub geometry: Arc<[SocGeometryReference]>,
}

pub(super) fn streams(
    descriptor: &SocContainerDescriptor<'_>,
    index: &PathIndex<'_>,
    mut check: impl FnMut() -> Result<(), SocCaptureError>,
) -> Result<Vec<SocStreamInput>, SocCaptureError> {
    descriptor
        .streams()
        .map(|stream| {
            check()?;
            let selector = SocMemberSelector::from_source(stream.declaration());
            let member = selector
                .path()
                .map(|path| SocMemberSelection::read(index, [path], &mut check))
                .transpose()?;
            Ok(SocStreamInput {
                selector,
                parent: stream.parent_position(),
                member,
                geometry: Arc::default(),
            })
        })
        .collect()
}

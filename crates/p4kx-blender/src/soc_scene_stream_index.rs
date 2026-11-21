// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{SocEntityIndex, SocMemberSelection, SocStreamInput};
use p4kx_extraction::{CancelToken, EntryOrigin};
use p4kx_model::SocEntityFile;

use crate::soc_scene::SocSceneError;

pub(super) fn read<'a>(
    declarations: &'a [SocStreamInput],
    cancel: &CancelToken,
) -> Result<(SocEntityIndex, BTreeMap<usize, &'a EntryOrigin>, bool), SocSceneError> {
    let mut by_member = BTreeMap::new();
    let mut complete = true;
    for declaration in declarations {
        if cancel.is_cancelled() {
            return Err(SocSceneError::Cancelled);
        }
        let Some(member) = declaration
            .member
            .as_ref()
            .and_then(SocMemberSelection::selected)
        else {
            complete = false;
            continue;
        };
        let physical_index = member.origin().entry().index();
        by_member.entry(physical_index).or_insert_with(|| {
            member
                .payload()
                .ok()
                .and_then(|bytes| SocEntityFile::parse(bytes).ok())
                .map(|file| (member.origin(), file))
        });
    }
    let mut index = SocEntityIndex::new();
    let mut admitted = BTreeMap::new();
    for (stream, parsed) in &by_member {
        let Some((origin, file)) = parsed else {
            complete = false;
            continue;
        };
        for chunk in file.entity_chunks() {
            if cancel.is_cancelled() {
                return Err(SocSceneError::Cancelled);
            }
            let Ok(chunk) = chunk else {
                complete = false;
                continue;
            };
            complete &= !chunk.document().is_lossy();
            index.add_chunk(*stream, &chunk);
        }
        admitted.insert(*stream, *origin);
    }
    Ok((index, admitted, complete))
}

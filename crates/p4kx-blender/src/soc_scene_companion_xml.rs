// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocMemberSelection;
use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};
use p4kx_extraction::{CancelToken, SourceUses};

use super::InputState;
use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes;
use crate::soc_scene_origin::XmlOrigin;
use crate::soc_scene_xml::XmlDocuments;

pub(super) fn read_selected<'a, T>(
    selection: &'a SocMemberSelection,
    sources: &'a SourceUses,
    parse: impl FnOnce(&'a [u8]) -> Option<T>,
) -> Result<Option<(T, XmlOrigin<'a>)>, SocSceneError> {
    let Some(member) = selection.selected() else {
        return Ok(None);
    };
    let Some(document) = member.payload().ok().and_then(parse) else {
        return Ok(None);
    };
    Ok(Some((
        document,
        XmlOrigin::new(sources, member.origin(), None)?,
    )))
}

pub(super) fn read<T>(
    bytes: &[u8],
    origin: &XmlOrigin<'_>,
    root: &str,
    documents: &mut XmlDocuments<'_>,
    cancel: &CancelToken,
    project: impl FnOnce(usize, &SourceDocument<'_>, &XmlOrigin<'_>) -> Result<T, SocSceneError>,
) -> Result<T, InputState> {
    soc_scene_classes::check(cancel).map_err(InputState::Capture)?;
    let document = SourceDocument::parse(bytes, SourceReadOptions::default())
        .map_err(|_| InputState::Invalid)?;
    let xml = documents
        .bind_checked(&document, origin, || soc_scene_classes::check(cancel))
        .map_err(InputState::Capture)?;
    let element = document.root();
    if document.is_lossy() || element.namespace().is_some() || element.name() != root {
        return Err(InputState::Unsupported(xml));
    }
    project(xml, &document, origin).map_err(InputState::Capture)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::sound::{WemListItem, WemListPayload, WemSourceChunk};

use crate::audio_wem_annotation_origins::{
    AnnotationOrigins, Site, WORD_BYTES, body_length, byte_order, parent_route, parent_tag,
};
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, audio_wem_text_origins};

pub(crate) fn header(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut origins = AnnotationOrigins::new(projection, writer)?;
    let indices = [parent.ordinal];
    let body = [parent.body_offset];
    let route = parent_route(&indices);
    origins.bind_occurrence(route, None)?;
    origins.fourcc(
        "/data/payload/list_type",
        "listType",
        parent.body_offset,
        &[route, parent_tag(&body)],
    )?;
    Ok(origins.finish())
}

pub(crate) fn item(
    projection: &WemProjection<'_>,
    parent: &WemSourceChunk,
    list_type: [u8; 4],
    item: &WemListItem,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut origins = AnnotationOrigins::new(projection, writer)?;
    let chunk = item.chunk();
    let sites = ListSites::new(parent, &chunk);
    let position = origins.bind_occurrence(sites.parent(), Some(sites.child()))?;
    origins.range("/data/payload/header_range", &position, &position)?;
    let extent = origins.capture(
        sites
            .routing()
            .into_iter()
            .chain([byte_order(), sites.body_length()]),
    )?;
    origins.range("/data/payload/body_range", &position, &extent)?;
    origins.fourcc(
        "/data/payload/tag",
        "listTag",
        chunk.header_offset,
        &sites.routing(),
    )?;
    padding(&mut origins, &sites, &extent)?;
    content(&mut origins, &sites, list_type, item.payload())?;
    Ok(origins.finish())
}

fn padding(
    origins: &mut AnnotationOrigins<'_, '_, '_>,
    sites: &ListSites<'_>,
    extent: &crate::origin_contributions::OriginContributions,
) -> Result<(), MediaError> {
    let end = sites.chunk.body_end_exclusive();
    let offsets = [end, sites.chunk.next_offset];
    let padding = (
        OriginSite::new("wemListDirectory", &offsets, "paddingDecision"),
        SourceUseRole::Selector,
    );
    let evidence = origins.capture(sites.routing().into_iter().chain([
        byte_order(),
        sites.body_length(),
        padding,
    ]))?;
    let path = "/data/payload/padding_range";
    if end < sites.chunk.next_offset {
        origins.range(path, extent, &evidence)
    } else {
        origins.field(path, evidence)
    }
}

fn content(
    origins: &mut AnnotationOrigins<'_, '_, '_>,
    sites: &ListSites<'_>,
    list_type: [u8; 4],
    value: &WemListPayload,
) -> Result<(), MediaError> {
    let (opaque, typed) = (
        matches!(value, WemListPayload::Opaque),
        list_type == *b"adtl",
    );
    let decision = origins.capture(
        sites
            .routing()
            .into_iter()
            .chain([sites.list_type()])
            .chain(typed.then(|| sites.child_tag()))
            .chain(
                (!opaque)
                    .then(|| [byte_order(), sites.body_length()])
                    .into_iter()
                    .flatten(),
            ),
    )?;
    origins.field("/data/payload/content", decision.clone())?;
    origins.field("/data/payload/content/type", decision.clone())?;
    match value {
        WemListPayload::Label(text) | WemListPayload::Note(text) => {
            audio_wem_text_origins::label(origins, sites, text)
        }
        WemListPayload::TextRegion(_) => audio_wem_text_origins::region(origins, sites),
        WemListPayload::Malformed { .. } => {
            origins.subtree("/data/payload/content/payload", decision.clone())?;
            origins.subtree("/issues", decision)
        }
        WemListPayload::Opaque => Ok(()),
        _ => Err(MediaError::Contract(
            "unrecognized WEM LIST evidence".into(),
        )),
    }
}

pub(crate) struct ListSites<'chunk> {
    pub(crate) chunk: &'chunk WemSourceChunk,
    parent_indices: [u64; 1],
    parent_body: [u64; 1],
    list_type_range: [u64; 2],
    child_indices: [u64; 2],
    child_body: [u64; 1],
    child_tag_range: [u64; 2],
}

impl<'chunk> ListSites<'chunk> {
    const fn new(parent: &WemSourceChunk, chunk: &'chunk WemSourceChunk) -> Self {
        Self {
            chunk,
            parent_indices: [parent.ordinal],
            parent_body: [parent.body_offset],
            list_type_range: [parent.body_offset, parent.body_offset + WORD_BYTES],
            child_indices: [parent.ordinal, chunk.ordinal],
            child_body: [chunk.body_offset],
            child_tag_range: [chunk.header_offset, chunk.header_offset + WORD_BYTES],
        }
    }

    const fn parent(&self) -> Site<'_> {
        parent_route(&self.parent_indices)
    }

    const fn child(&self) -> Site<'_> {
        (
            OriginSite::new("wemListDirectory", &self.child_indices, "chunkRouting"),
            SourceUseRole::Selector,
        )
    }

    const fn list_type(&self) -> Site<'_> {
        (
            OriginSite::new("wemAnnotationField", &self.list_type_range, "listType"),
            SourceUseRole::Selector,
        )
    }

    const fn child_tag(&self) -> Site<'_> {
        (
            OriginSite::new("wemAnnotationField", &self.child_tag_range, "listTag"),
            SourceUseRole::Selector,
        )
    }

    pub(crate) const fn body_length(&self) -> Site<'_> {
        body_length(&self.child_body)
    }

    const fn routing(&self) -> [Site<'_>; 3] {
        [self.parent(), parent_tag(&self.parent_body), self.child()]
    }

    pub(crate) const fn recognized(&self) -> [Site<'_>; 5] {
        let [parent, tag, child] = self.routing();
        [parent, tag, child, self.list_type(), self.child_tag()]
    }
    pub(crate) fn recognized_with_extent(&self) -> impl Iterator<Item = Site<'_>> {
        self.recognized()
            .into_iter()
            .chain([byte_order(), self.body_length()])
    }
}

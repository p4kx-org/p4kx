// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;

use crate::audio_record_origins as origins;
use crate::audio_wem_projection::WemProjection;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) type Site<'a> = (OriginSite<'a>, SourceUseRole);

pub(crate) const WORD_BYTES: u64 = size_of::<u32>() as u64;
pub(crate) const HALF_WORD_BYTES: u64 = size_of::<u16>() as u64;

pub(crate) struct AnnotationOrigins<'source, 'writer, 'run> {
    projection: &'source WemProjection<'source>,
    writer: &'writer MediaWriter<'run>,
    prepared: PreparedRecordOrigins,
}

impl<'source, 'writer, 'run> AnnotationOrigins<'source, 'writer, 'run> {
    pub(crate) fn new(
        projection: &'source WemProjection<'source>,
        writer: &'writer MediaWriter<'run>,
    ) -> Result<Self, MediaError> {
        Ok(Self {
            projection,
            writer,
            prepared: origins::prepare(projection.input)?,
        })
    }

    pub(crate) fn capture<'site>(
        &self,
        sites: impl IntoIterator<Item = Site<'site>>,
    ) -> Result<OriginContributions, MediaError> {
        self.projection.capture(sites, self.writer)
    }

    pub(crate) fn field(
        &mut self,
        path: &str,
        evidence: OriginContributions,
    ) -> Result<(), MediaError> {
        origins::field(&mut self.prepared, path, evidence, self.writer)
    }

    pub(crate) fn subtree(
        &mut self,
        path: &str,
        evidence: OriginContributions,
    ) -> Result<(), MediaError> {
        origins::subtree(&mut self.prepared, path, evidence, self.writer)
    }

    pub(crate) fn range(
        &mut self,
        path: &str,
        start: &OriginContributions,
        end: &OriginContributions,
    ) -> Result<(), MediaError> {
        origins::range(&mut self.prepared, path, start, end, self.writer)
    }

    pub(crate) fn indices(&mut self, evidence: OriginContributions) -> Result<(), MediaError> {
        self.projection
            .indices(&mut self.prepared, evidence, self.writer)
    }

    pub(crate) fn bind_occurrence(
        &mut self,
        parent: Site<'_>,
        child: Option<Site<'_>>,
    ) -> Result<OriginContributions, MediaError> {
        let parent_origin = self.capture([parent])?;
        self.field("/data/payload/chunk_ordinal", parent_origin.clone())?;
        let Some(child) = child else {
            self.indices(parent_origin.clone())?;
            return Ok(parent_origin);
        };
        let parent_path = format!("/subresource/indices/{}", self.projection.index_offset());
        self.field(&parent_path, parent_origin)?;
        let position = self.capture([parent, child])?;
        self.field("/data/payload/ordinal", position.clone())?;
        self.indices(position.clone())?;
        Ok(position)
    }

    pub(crate) fn word(
        &mut self,
        path: &str,
        name: &str,
        start: u64,
        width: u64,
        route: &[Site<'_>],
    ) -> Result<(), MediaError> {
        let range = [start, start + width];
        let actual = (
            OriginSite::new("wemAnnotationField", &range, name),
            SourceUseRole::Payload,
        );
        let evidence = self.capture(route.iter().copied().chain([actual]))?;
        self.field(path, evidence)
    }

    pub(crate) fn fourcc(
        &mut self,
        path: &str,
        name: &str,
        start: u64,
        route: &[Site<'_>],
    ) -> Result<(), MediaError> {
        self.word(path, name, start, WORD_BYTES, route)?;
        for index in 0..WORD_BYTES {
            let field = format!("{path}/{index}");
            self.word(&field, name, start + index, 1, route)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> PreparedRecordOrigins {
        self.prepared
    }
}

pub(crate) const fn parent_route(indices: &[u64]) -> Site<'_> {
    (
        OriginSite::new("wemRiffDirectory", indices, "chunkRouting"),
        SourceUseRole::Selector,
    )
}

pub(crate) const fn parent_tag(body: &[u64]) -> Site<'_> {
    (
        OriginSite::new("wemChunk", body, "id"),
        SourceUseRole::Selector,
    )
}

pub(crate) const fn byte_order() -> Site<'static> {
    (
        OriginSite::new("wemRiffHeader", &[], "signature"),
        SourceUseRole::Conversion,
    )
}

pub(crate) const fn body_length(body: &[u64]) -> Site<'_> {
    (
        OriginSite::new("wemChunk", body, "bodyLength"),
        SourceUseRole::Conversion,
    )
}

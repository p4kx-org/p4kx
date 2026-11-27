// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::sound::{WemCueText, WemTextUtf8};

use crate::MediaError;
use crate::audio_wem_annotation_origins::{
    AnnotationOrigins, HALF_WORD_BYTES, Site, WORD_BYTES, byte_order,
};
use crate::audio_wem_list_origins::ListSites;
use crate::origin_capture::OriginSite;

pub(crate) fn label(
    origins: &mut AnnotationOrigins<'_, '_, '_>,
    sites: &ListSites<'_>,
    text: &WemCueText,
) -> Result<(), MediaError> {
    let recognized = sites.recognized();
    let [parent, parent_tag, child, list_type, child_tag] = recognized;
    let numeric = [
        parent,
        parent_tag,
        child,
        list_type,
        child_tag,
        byte_order(),
    ];
    origins.word(
        "/data/payload/content/payload/cue_id",
        "labelCueId",
        sites.chunk.body_offset,
        WORD_BYTES,
        &numeric,
    )?;
    let start = origins.capture(recognized)?;
    let extent = origins.capture(sites.recognized_with_extent())?;
    origins.range("/data/payload/content/payload/raw_range", &start, &extent)?;
    let terminator = [
        text.raw_range.start,
        text.terminator_offset
            .map_or(text.raw_range.end, |offset| offset + 1),
    ];
    let nul = (terminator[0] != terminator[1]).then_some((
        OriginSite::new("wemAnnotationText", &terminator, "nulScan"),
        SourceUseRole::Selector,
    ));
    let ending = origins.capture(sites.recognized_with_extent().chain(nul))?;
    origins.range("/data/payload/content/payload/text_range", &start, &ending)?;
    origins.field("/data/payload/content/payload/terminator_offset", ending)?;
    utf8(origins, sites, text, nul, &start)
}

fn utf8(
    origins: &mut AnnotationOrigins<'_, '_, '_>,
    sites: &ListSites<'_>,
    text: &WemCueText,
    nul: Option<Site<'_>>,
    start: &crate::origin_contributions::OriginContributions,
) -> Result<(), MediaError> {
    let validated = [text.utf8_input_range.start, text.utf8_input_range.end];
    let validation = (validated[0] != validated[1]).then_some((
        OriginSite::new("wemAnnotationText", &validated, "utf8Validation"),
        SourceUseRole::Conversion,
    ));
    let interpretation = nul.filter(|_| text.utf8 != WemTextUtf8::Invalid);
    let evidence = origins.capture(
        sites
            .recognized_with_extent()
            .chain(validation)
            .chain(interpretation),
    )?;
    origins.range(
        "/data/payload/content/payload/utf8_input_range",
        start,
        &evidence,
    )?;
    origins.field("/data/payload/content/payload/utf8", evidence.clone())?;
    let range = [text.text_range.start, text.text_range.end];
    let value_read = (text.utf8 != WemTextUtf8::Invalid && range[0] != range[1]).then_some((
        OriginSite::new("wemAnnotationText", &range, "utf8Text"),
        SourceUseRole::Conversion,
    ));
    let value = origins.capture(
        sites
            .recognized_with_extent()
            .chain(validation)
            .chain(interpretation)
            .chain(value_read),
    )?;
    origins.field("/data/payload/content/payload/utf8_text", value)
}

pub(crate) fn region(
    origins: &mut AnnotationOrigins<'_, '_, '_>,
    sites: &ListSites<'_>,
) -> Result<(), MediaError> {
    let start = sites.chunk.body_offset;
    let recognized = sites.recognized();
    region_words(origins, start, recognized)?;
    origins.fourcc(
        "/data/payload/content/payload/purpose",
        "purpose",
        start + 2 * WORD_BYTES,
        &recognized,
    )?;
    let extent = origins.capture(sites.recognized_with_extent())?;
    let position = origins.capture(recognized)?;
    origins.range(
        "/data/payload/content/payload/text_range",
        &position,
        &extent,
    )
}

fn region_words(
    origins: &mut AnnotationOrigins<'_, '_, '_>,
    start: u64,
    [parent, parent_tag, child, list_type, child_tag]: [Site<'_>; 5],
) -> Result<(), MediaError> {
    let numeric = [
        parent,
        parent_tag,
        child,
        list_type,
        child_tag,
        byte_order(),
    ];
    for (path, name, offset, width) in [
        (
            "/data/payload/content/payload/cue_id",
            "regionCueId",
            0,
            WORD_BYTES,
        ),
        (
            "/data/payload/content/payload/sample_length",
            "sampleLength",
            4,
            WORD_BYTES,
        ),
        (
            "/data/payload/content/payload/country",
            "country",
            12,
            HALF_WORD_BYTES,
        ),
        (
            "/data/payload/content/payload/language",
            "language",
            14,
            HALF_WORD_BYTES,
        ),
        (
            "/data/payload/content/payload/dialect",
            "dialect",
            16,
            HALF_WORD_BYTES,
        ),
        (
            "/data/payload/content/payload/code_page",
            "codePage",
            18,
            HALF_WORD_BYTES,
        ),
    ] {
        origins.word(path, name, start + offset, width, &numeric)?;
    }
    Ok(())
}

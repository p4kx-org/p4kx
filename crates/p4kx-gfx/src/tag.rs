// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::reader::TagReader;
use crate::{FailureOrigin, GfxError, MovieSpan, TagSite};

const LONG_LENGTH: u16 = 0x3F;

/// One tag of a tag stream: its code, its body and where the body starts in the movie data.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tag<'a> {
    pub(crate) code: u16,
    pub(crate) body: &'a [u8],
    pub(crate) offset: usize,
    header_start: usize,
}

impl<'a> Tag<'a> {
    pub(crate) const fn site(&self) -> TagSite {
        TagSite {
            code: self.code,
            header: MovieSpan::new(self.header_start, self.offset),
            body: MovieSpan::new(self.offset, self.offset + self.body.len()),
        }
    }

    /// Returns a reader over the body.
    pub(crate) const fn reader(&self) -> TagReader<'a> {
        TagReader::new(self.body, self.offset)
    }
}

pub(crate) struct TagReadFailure {
    pub(crate) error: GfxError,
    pub(crate) origin: FailureOrigin,
}

/// Reads the tags of a stream one by one until its `End` tag or its end.
#[derive(Debug, Clone)]
pub(crate) struct TagStream<'a> {
    bytes: &'a [u8],
    position: usize,
    base: usize,
    ended: bool,
}

impl<'a> TagStream<'a> {
    /// Creates a stream over `bytes`, which start at `base` in the movie data.
    pub(crate) const fn new(bytes: &'a [u8], base: usize) -> Self {
        Self {
            bytes,
            position: 0,
            base,
            ended: false,
        }
    }

    pub(crate) const fn offset(&self) -> usize {
        self.base + self.position
    }

    /// Reads the next tag, `None` after the `End` tag or at the end of the data.
    ///
    /// A tag header or body that runs past the data is an error, after which the stream ends.
    pub(crate) fn next_tag(&mut self) -> Option<Result<Tag<'a>, TagReadFailure>> {
        if self.ended || self.position >= self.bytes.len() {
            return None;
        }
        let rest = self.bytes.get(self.position..).unwrap_or_default();
        let header_offset = self.base + self.position;
        let result = read_tag(rest, header_offset);
        match &result {
            Ok((tag, consumed)) => {
                self.position += consumed;
                self.ended = tag.code == 0;
            }
            Err(_) => self.ended = true,
        }
        Some(result.map(|(tag, _)| tag))
    }
}

fn read_tag(bytes: &[u8], offset: usize) -> Result<(Tag<'_>, usize), TagReadFailure> {
    let mut reader = TagReader::new(bytes, offset);
    let fields = read_tag_header(&mut reader);
    let body_offset = reader.offset();
    let failure = |error| TagReadFailure {
        error,
        origin: FailureOrigin::Stream {
            header: MovieSpan::new(offset, body_offset),
            available: MovieSpan::new(body_offset, offset + bytes.len()),
        },
    };
    let (code, len) = fields.map_err(failure)?;
    let header_len = body_offset - offset;
    let available = bytes.len() - header_len;
    let body = usize::try_from(len)
        .ok()
        .and_then(|len| bytes.get(header_len..header_len.checked_add(len)?))
        .ok_or_else(|| {
            failure(GfxError::TagLength {
                code,
                offset,
                len,
                available,
            })
        })?;
    let tag = Tag {
        code,
        body,
        offset: body_offset,
        header_start: offset,
    };
    Ok((tag, header_len + body.len()))
}

fn read_tag_header(reader: &mut TagReader<'_>) -> Result<(u16, u32), GfxError> {
    let code_and_length = reader.u16()?;
    let code = code_and_length >> 6;
    let short = code_and_length & LONG_LENGTH;
    let len = if short == LONG_LENGTH {
        reader.u32()?
    } else {
        u32::from(short)
    };
    Ok((code, len))
}

/// Returns the name of a SWF or GFx tag code, `None` for a code this crate does not know.
#[must_use]
#[expect(clippy::too_many_lines, reason = "one arm per tag code")]
pub const fn tag_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0 => "End",
        1 => "ShowFrame",
        2 => "DefineShape",
        4 => "PlaceObject",
        5 => "RemoveObject",
        6 => "DefineBits",
        7 => "DefineButton",
        8 => "JPEGTables",
        9 => "SetBackgroundColor",
        10 => "DefineFont",
        11 => "DefineText",
        12 => "DoAction",
        13 => "DefineFontInfo",
        14 => "DefineSound",
        15 => "StartSound",
        17 => "DefineButtonSound",
        18 => "SoundStreamHead",
        19 => "SoundStreamBlock",
        20 => "DefineBitsLossless",
        21 => "DefineBitsJPEG2",
        22 => "DefineShape2",
        23 => "DefineButtonCxform",
        24 => "Protect",
        26 => "PlaceObject2",
        28 => "RemoveObject2",
        32 => "DefineShape3",
        33 => "DefineText2",
        34 => "DefineButton2",
        35 => "DefineBitsJPEG3",
        36 => "DefineBitsLossless2",
        37 => "DefineEditText",
        39 => "DefineSprite",
        41 => "ProductInfo",
        43 => "FrameLabel",
        45 => "SoundStreamHead2",
        46 => "DefineMorphShape",
        48 => "DefineFont2",
        56 => "ExportAssets",
        57 => "ImportAssets",
        58 => "EnableDebugger",
        59 => "DoInitAction",
        60 => "DefineVideoStream",
        61 => "VideoFrame",
        62 => "DefineFontInfo2",
        63 => "DebugID",
        64 => "EnableDebugger2",
        65 => "ScriptLimits",
        66 => "SetTabIndex",
        69 => "FileAttributes",
        70 => "PlaceObject3",
        71 => "ImportAssets2",
        72 => "DoABCDefine",
        73 => "DefineFontAlignZones",
        74 => "CSMTextSettings",
        75 => "DefineFont3",
        76 => "SymbolClass",
        77 => "Metadata",
        78 => "DefineScalingGrid",
        82 => "DoABC",
        83 => "DefineShape4",
        84 => "DefineMorphShape2",
        86 => "DefineSceneAndFrameLabelData",
        87 => "DefineBinaryData",
        88 => "DefineFontName",
        89 => "StartSound2",
        90 => "DefineBitsJPEG4",
        91 => "DefineFont4",
        93 => "EnableTelemetry",
        94 => "PlaceObject4",
        1000 => "ExporterInfo",
        1001 => "DefineExternalImage",
        1002 => "FontTextureInfo",
        1003 => "DefineExternalGradient",
        1004 => "DefineGradientMap",
        1005 => "DefineCompactedFont",
        1006 => "DefineExternalSound",
        1007 => "DefineExternalStreamSound",
        1008 => "DefineSubImage",
        1009 => "DefineExternalImage2",
        _ => return None,
    })
}

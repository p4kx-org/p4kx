// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_testkit::{
    TestGlyph, TestMovie, TestSignature, exporter_info_body, exports_body, external_image_body,
    font_name_body, font3_body, import_assets_body, place_object2_body, scaling_grid_body,
    sprite_body, symbol_class_body, tag_bytes,
};

use super::{FONT, TestResult};

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    offsets: BTreeMap<u16, Vec<usize>>,
}

impl Fixture {
    pub(super) fn offset(&self, code: u16, ordinal: usize) -> TestResult<String> {
        self.offsets
            .get(&code)
            .and_then(|offsets| offsets.get(ordinal))
            .map(usize::to_string)
            .ok_or_else(|| format!("tag {code}, ordinal {ordinal}").into())
    }
}

fn font_info(name: &str, flags: u8) -> TestResult<Vec<u8>> {
    let mut bytes = FONT.to_le_bytes().to_vec();
    bytes.push(u8::try_from(name.len())?);
    bytes.extend_from_slice(name.as_bytes());
    bytes.extend([flags, 65]);
    Ok(bytes)
}

fn nested_timeline() -> Vec<u8> {
    let inner = sprite_body(10, 1, &[(1, Vec::new())]);
    sprite_body(
        9,
        3,
        &[
            (26, place_object2_body(1, FONT)),
            (26, place_object2_body(2, FONT)),
            (1, Vec::new()),
            (43, b"after-one\0".to_vec()),
            (39, inner),
            (1, Vec::new()),
        ],
    )
}

pub(super) fn fixture(signature: TestSignature) -> TestResult<Fixture> {
    let glyph = TestGlyph {
        code: 65,
        advance: 16,
        records: Vec::new(),
    };
    let first = font3_body(FONT, "Original", std::slice::from_ref(&glyph));
    let second = font3_body(FONT, "Duplicate", &[glyph]);
    let tags = [
        (1000, exporter_info_body("First"), false),
        (75, first, false),
        (75, second, false),
        (13, font_info("Earlier", 0)?, false),
        (13, font_info("Latest", 6)?, true),
        (88, font_name_body(FONT, "Earlier full"), false),
        (88, font_name_body(FONT, "Latest full"), false),
        (56, exports_body(&[(99, "other"), (FONT, "winner")]), false),
        (56, exports_body(&[(FONT, "unused")]), false),
        (
            1009,
            external_image_body(6, 2, 14, (4, 2), ("tex", "never-read.dds")),
            false,
        ),
        (
            57,
            import_assets_body("never-read.swf", 1, &[(1, "one"), (2, "two")]),
            false,
        ),
        (
            76,
            symbol_class_body(&[(1, "ClassOne"), (2, "ClassTwo")]),
            false,
        ),
        (39, nested_timeline(), false),
        (78, scaling_grid_body(9, [10, 100, 15, 90]), false),
        (1000, exporter_info_body("Last"), false),
        (777, b"unknown".to_vec(), false),
        (69, vec![0; 4], false),
        (1, Vec::new(), false),
        (2, vec![4, 0], false),
    ];
    let mut movie = TestMovie::new(signature).frame_count(9);
    let mut offset = 8 + movie.body().len() - tag_bytes(0, &[]).len();
    let mut offsets = BTreeMap::<u16, Vec<usize>>::new();
    for (code, body, long) in tags {
        offsets.entry(code).or_default().push(offset);
        if long {
            offset += size_of::<u16>() + size_of::<u32>() + body.len();
            movie = movie.long_tag(code, &body);
        } else {
            offset += tag_bytes(code, &body).len();
            movie = movie.tag(code, &body);
        }
    }
    Ok(Fixture {
        bytes: movie.bytes(),
        offsets,
    })
}

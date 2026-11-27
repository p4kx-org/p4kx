// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// One authored DDS source value and its exact location in the original header.
#[derive(Debug)]
#[non_exhaustive]
pub struct DdsSourceWord {
    /// Relative field address inside the raw DDS metadata object.
    pub field: String,
    /// Canonical original source word label.
    pub label: &'static str,
    /// Zero-based offset from the start of the original DDS Entry or file.
    pub offset: u64,
    /// Number of original bytes used for the value.
    pub length: u64,
    /// Whether the DX10 format selector contributes to the field's presence.
    pub dx10: bool,
}

const LEGACY_WORDS: &[&str] = &[
    "size",
    "flags",
    "height",
    "width",
    "pitch_or_linear_size",
    "depth",
    "mip_map_count",
    "reserved1/0",
    "reserved1/1",
    "reserved1/2",
    "reserved1/3",
    "reserved1/4",
    "reserved1/5",
    "reserved1/6",
    "reserved1/7",
    "reserved1/8",
    "reserved1/9",
    "reserved1/10",
    "pixel_format/size",
    "pixel_format/flags",
    "pixel_format/four_cc",
    "pixel_format/rgb_bit_count",
    "pixel_format/r_bit_mask",
    "pixel_format/g_bit_mask",
    "pixel_format/b_bit_mask",
    "pixel_format/a_bit_mask",
    "caps",
    "caps2",
    "caps3",
    "caps4",
    "reserved2",
];
const DX10_WORDS: &[&str] = &[
    "dxgi_format",
    "resource_dimension",
    "misc_flag",
    "array_size",
    "misc_flags2",
];

/// Enumerates the exact source words of the shared raw DDS wire projection.
pub fn raw_dds_source_words(
    magic_present: bool,
    dx10_present: bool,
) -> impl Iterator<Item = DdsSourceWord> {
    let magic = u64::from(magic_present) * 4;
    let legacy = LEGACY_WORDS
        .iter()
        .enumerate()
        .flat_map(move |(index, field)| {
            let offset = magic + index as u64 * 4;
            let label = if field.starts_with("reserved1/") {
                "reserved1"
            } else if *field == "pixel_format/four_cc" {
                "pixelFormat.fourCc"
            } else {
                field
            };
            let parts = if *field == "pixel_format/four_cc" {
                4
            } else {
                0
            };
            let whole = DdsSourceWord {
                field: format!("legacy/{field}"),
                label,
                offset,
                length: 4,
                dx10: false,
            };
            std::iter::once(whole).chain((0..parts).map(move |byte| DdsSourceWord {
                field: format!("legacy/{field}/{byte}"),
                label,
                offset: offset + byte,
                length: 1,
                dx10: false,
            }))
        });
    let extension = DX10_WORDS
        .iter()
        .enumerate()
        .filter(move |_| dx10_present)
        .map(move |(index, field)| DdsSourceWord {
            field: format!("dx10/{field}"),
            label: field,
            offset: magic + 124 + index as u64 * 4,
            length: 4,
            dx10: true,
        });
    legacy.chain(extension)
}

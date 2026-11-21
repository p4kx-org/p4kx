// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{SplitDds, TestTextureFormat};
use p4kx_texture::{MergeSpan, MergeSpanSite};

use super::{Parts, merged};

#[test]
fn observed_ranges_follow_the_written_split_and_unsplit_cube() -> Result<(), Box<dyn Error>> {
    let fixture = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4).cubemap();
    for split in [0, 2] {
        let dds = fixture.clone().split(split);
        let parts = Parts::of(&dds);
        let mut header = dds.header_bytes();
        header.extend_from_slice(b"CExtCEnd");
        let mut actual = Vec::new();
        let mut spans = Vec::new();
        let report = p4kx_texture::merge_with_spans(
            header.as_slice(),
            header.len() as u64,
            &parts,
            &mut actual,
            |span| {
                spans.push(span);
                Ok(())
            },
        )?;
        assert_eq!(
            actual,
            merged(&dds, &parts)?,
            "observing preserves all ordinary bytes"
        );
        assert_eq!(
            report.size,
            actual.len() as u64,
            "the checked trailer stays out"
        );
        verify_cube_spans(&spans, &dds, &header, &actual)?;
    }
    Ok(())
}

fn verify_cube_spans(
    spans: &[MergeSpan],
    dds: &SplitDds,
    header: &[u8],
    actual: &[u8],
) -> Result<(), Box<dyn Error>> {
    let (first, surfaces) = spans.split_first().ok_or("original DDS header span")?;
    assert_eq!(
        first.site,
        MergeSpanSite::Header,
        "the raw header comes first"
    );
    assert_eq!(
        (first.input_start, first.output_start, first.byte_length),
        (0, 0, 128),
        "complete original legacy DDS header"
    );
    assert_eq!(surfaces.len(), 18, "three mips on each of six faces");
    let (faces, remainder) = surfaces.as_chunks::<3>();
    assert!(remainder.is_empty(), "fixture has three mips per face");
    for (face, mips) in faces.iter().enumerate() {
        for (level, (&span, (prefix, size))) in
            mips.iter().zip([(0, 64), (64, 16), (80, 4)]).enumerate()
        {
            let expected_output = 128 + 84 * face + prefix;
            let (part, expected_input) = match (dds.part_count(), level) {
                (2, 0) => (Some(2), 64 * face),
                (2, 1) => (Some(1), 16 * face),
                (2, 2) => (None, 128 + 4 * face),
                _ => (None, expected_output),
            };
            let site = MergeSpanSite::Mip {
                level: u32::try_from(level)?,
                surface: face as u64,
                part,
            };
            assert_eq!(
                span.site, site,
                "face {face} mip {level} uses its original source"
            );
            assert_eq!(
                span.input_start, expected_input as u64,
                "original input coordinate"
            );
            assert_eq!(
                span.output_start, expected_output as u64,
                "emitted output coordinate"
            );
            assert_eq!(span.byte_length, size as u64, "the complete surface");
            let input_bytes = part.map(|part| dds.part(part));
            let input = input_bytes.as_deref().unwrap_or(header);
            assert_eq!(
                input.get(expected_input..expected_input + size),
                actual.get(expected_output..expected_output + size),
                "the exact bytes at both coordinates for {site:?}"
            );
        }
    }
    Ok(())
}

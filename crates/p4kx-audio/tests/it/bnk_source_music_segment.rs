// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{Cursor, Read, Seek};

use p4kx_audio::{
    BankHircSemantic, BankMusicSegmentRead, BankMusicSegmentRow, BankMusicSegmentRows, BankSource,
    HircValue, MusicSegment, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SEGMENT_TYPE: u8 = 10;
const SEGMENT_ID: u32 = 77;

#[test]
fn music_segment_source_replays_legacy_fields_and_exact_marker_name() -> TestResult {
    let object = TestHircObject::music_segment(SEGMENT_ID, &[21, 21], &["Entry", "Exit"]);
    let bytes = TestBank::new(1).object(object).bytes();
    let legacy = Soundbank::parse(&bytes)?;
    let original = legacy.objects().first().ok_or("legacy segment")?;
    let HircValue::MusicSegment(expected) = &original.value else {
        return Err("decoded slice segment".into());
    };
    let mut source = BankSource::open(Cursor::new(&bytes), u64::try_from(bytes.len())?)?;
    let mut sections = source.section_cursor();
    let header = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hierarchy = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    let header = source.read_observed_header_for(&header)?;
    let mut hirc = source.hirc(&hierarchy)?;
    let Some(BankHircSemantic::MusicSegment(segment)) = hirc.next_semantic(&header).transpose()?
    else {
        return Err("typed MusicSegment".into());
    };
    assert!(
        matches!(segment.status, BankMusicSegmentRead::Decoded),
        "the complete body decodes"
    );
    assert_eq!(
        segment.fields.duration.map(|word| word.value),
        Some(expected.duration.to_bits()),
        "the authored duration matches the slice reader"
    );
    assert_eq!(
        segment.fields.node.grid_period.map(|word| word.value),
        Some(expected.node.meter.grid_period.to_bits()),
        "the period matches the slice reader bit for bit"
    );
    assert_eq!(
        segment.fields.node.grid_offset.map(|word| word.value),
        Some(expected.node.meter.grid_offset.to_bits()),
        "the offset matches the slice reader bit for bit"
    );
    check_rows(segment.rows, &bytes, expected)
}

fn check_rows<R: Read + Seek>(
    mut rows: BankMusicSegmentRows<'_, R>,
    bytes: &[u8],
    expected: &MusicSegment,
) -> TestResult {
    let mut children = Vec::new();
    let mut stingers = Vec::new();
    let mut markers = Vec::new();
    while let Some(row) = rows.next() {
        match row? {
            BankMusicSegmentRow::Child { id, .. } => children.push(id.value),
            BankMusicSegmentRow::Stinger { value, .. } => {
                stingers.push((value.trigger_id.value, value.segment_id.value));
            }
            BankMusicSegmentRow::Marker { ordinal, value } => {
                let position = usize::try_from(ordinal)?;
                let marker = expected.markers.get(position).ok_or("legacy marker")?;
                let mut name = vec![0; marker.name.len()];
                rows.read_current_name(&mut name)?;
                let start = usize::try_from(value.name_range.start)?;
                let end = usize::try_from(value.name_range.end)?;
                assert_eq!(
                    bytes.get(start..end),
                    Some(name.as_slice()),
                    "the name range matches the original marker bytes"
                );
                assert_eq!(
                    bytes.get(end),
                    Some(&0),
                    "the source byte after the name is its terminator"
                );
                assert_eq!(
                    name,
                    marker.name.as_bytes(),
                    "both readers return the same marker name"
                );
                assert_eq!(value.id.value, marker.id, "the marker ID is unchanged");
                assert_eq!(
                    value.position.value,
                    marker.position.to_bits(),
                    "the marker position keeps its original bits"
                );
                markers.push(ordinal);
            }
            _ => return Err("unexpected original segment row".into()),
        }
    }
    assert_eq!(
        children, expected.node.children,
        "original child order and duplicates survive"
    );
    assert_eq!(
        stingers,
        expected
            .node
            .stingers
            .iter()
            .map(|value| (value.trigger_id, value.segment_id))
            .collect::<Vec<_>>(),
        "stinger identities and order match the slice reader"
    );
    assert_eq!(markers, [0, 1], "marker positions follow the original list");
    Ok(())
}

#[test]
fn unterminated_last_marker_leaves_previous_original_rows_available() -> TestResult {
    let valid = TestBank::new(1)
        .object(TestHircObject::music_segment(
            SEGMENT_ID,
            &[21],
            &["Entry", "Exit"],
        ))
        .bytes();
    let parsed = Soundbank::parse(&valid)?;
    let object = parsed.objects().first().ok_or("legacy segment")?;
    let body = parsed.object_body(object);
    let body = body.get(4..body.len() - 1).ok_or("omit final marker NUL")?;
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(SEGMENT_TYPE, SEGMENT_ID, body))
        .bytes();
    let mut source = BankSource::open(Cursor::new(&bytes), u64::try_from(bytes.len())?)?;
    let mut sections = source.section_cursor();
    let header = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hierarchy = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    let header = source.read_observed_header_for(&header)?;
    let mut hirc = source.hirc(&hierarchy)?;
    let Some(BankHircSemantic::MusicSegment(segment)) = hirc.next_semantic(&header).transpose()?
    else {
        return Err("typed malformed MusicSegment".into());
    };
    assert!(
        matches!(segment.status, BankMusicSegmentRead::Malformed(_)),
        "the unterminated suffix is malformed"
    );
    assert_eq!(
        segment.fields.marker_count.map(|word| word.value),
        Some(2),
        "the original count includes the malformed marker"
    );
    let markers = segment.rows.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        markers
            .iter()
            .filter(|row| matches!(row, BankMusicSegmentRow::Marker { .. }))
            .count(),
        1,
        "the first complete marker remains readable"
    );
    Ok(())
}

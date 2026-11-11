// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;

use p4kx_audio::{
    BankActionRead, BankError, BankHirc, BankHircAction, BankHircField, BankObservedHeader,
    BankPlayBankDecision, BankReadError, BankSource, BankSourceSection, HircKind, HircValue,
    SectionId, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const ACTION_ID: u32 = 71;
const TARGET: u32 = 93;
const PLAY: u16 = 0x0403;
const STOP: u16 = 0x0102;
const BANK_TYPE_OFFSET: u64 = 106;

#[test]
fn bank_source_action_reads_exact_play_fields_and_skips_opaque_properties() -> TestResult {
    let bytes = TestBank::new(12)
        .object(action(PLAY, 4))
        .object(action(STOP, 4))
        .object(TestHircObject::event(72, &[]))
        .bytes();
    let trace = RefCell::new(Vec::new());
    let mut bank = traced_bank(&bytes, &trace, None)?;
    let (header, section) = header_and_hirc(&mut bank)?;
    assert_eq!(header.section().header_offset(), 0, "actual BKHD section");
    assert_eq!(header.header().version, 154, "actual BKHD version word");
    let mut hirc = bank.hirc(&section)?;
    trace.borrow_mut().clear();

    let first = next(&mut hirc, &header)?;
    assert_eq!(first.object.id(), ACTION_ID, "original Action ID");
    assert_eq!(first.object.ordinal(), 0, "original first object");
    check_play_fields(first.status)?;
    assert_eq!(
        trace.borrow().as_slice(),
        [
            64..69,
            69..73,
            73..75,
            75..79,
            79..80,
            80..81,
            91..92,
            101..102,
            102..106,
            106..110
        ],
        "physical header, ID and actual fields; no opaque property bodies read"
    );
    let second = next(&mut hirc, &header)?;
    assert_eq!(second.object.id(), ACTION_ID, "duplicate short ID");
    assert_eq!(second.object.ordinal(), 1, "second original occurrence");
    check_non_play(second.status)?;
    assert!(
        matches!(next(&mut hirc, &header)?.status, BankActionRead::NotAction),
        "the physical Event has no Action payload"
    );
    check_legacy_actions(&bytes)?;
    Ok(())
}

#[test]
fn bank_source_action_keeps_version_gated_and_unsupported_paths_distinct() -> TestResult {
    for version in [153, 155] {
        let bytes = TestBank::new(12)
            .version(version)
            .object(action(PLAY, 4))
            .bytes();
        let trace = RefCell::new(Vec::new());
        let mut bank = traced_bank(&bytes, &trace, None)?;
        let (header, section) = header_and_hirc(&mut bank)?;
        let mut hirc = bank.hirc(&section)?;
        trace.borrow_mut().clear();
        let item = next(&mut hirc, &header)?;
        let legacy = Soundbank::parse(bytes.as_slice())?;
        let original = legacy.objects().first().ok_or("legacy Action occurrence")?;
        match (version, item.status) {
            (153, BankActionRead::Decoded { target, fields }) => {
                assert_eq!(
                    original.value,
                    HircValue::Action(target),
                    "legacy equivalence"
                );
                assert_eq!(target.bank, None, "pre-154 Play has no PlayBank");
                assert!(
                    matches!(fields.play_bank, BankPlayBankDecision::VersionGated),
                    "version gate"
                );
                assert_eq!(
                    trace.borrow().as_slice(),
                    [64..69, 69..73, 73..75, 75..79],
                    "decoy fields were not consulted"
                );
            }
            (155, BankActionRead::Unsupported { error }) => {
                assert_eq!(
                    error,
                    BankError::UnsupportedObjectLayout {
                        kind: HircKind::Action,
                        version: 155
                    },
                    "original version is outside the known Action layout"
                );
                assert_eq!(original.error(), Some(&error), "legacy error equivalence");
                assert_eq!(
                    trace.borrow().as_slice(),
                    [64..69, 69..73],
                    "only the raw envelope"
                );
            }
            _ => return Err("unexpected version-specific Action state".into()),
        }
    }
    Ok(())
}

#[test]
fn bank_source_action_keeps_malformed_prefix_and_legacy_object_relative_error() -> TestResult {
    let bytes = TestBank::new(12)
        .object(action(PLAY, 2))
        .object(action(STOP, 4))
        .bytes();
    let mut bank = BankSource::open(Cursor::new(bytes.as_slice()), bytes.len() as u64)?;
    let (header, section) = header_and_hirc(&mut bank)?;
    let mut hirc = bank.hirc(&section)?;
    let BankActionRead::Malformed { error, fields } = next(&mut hirc, &header)?.status else {
        return Err("malformed PlayBank expected".into());
    };
    assert_eq!(
        error,
        BankError::Truncated {
            section: SectionId::HIRC,
            len: 39,
            needed: 41
        },
        "extent is relative to the object body including the ID"
    );
    let legacy = Soundbank::parse(bytes.as_slice())?;
    assert_eq!(
        legacy
            .objects()
            .first()
            .ok_or("original malformed Action")?
            .error(),
        Some(&error),
        "legacy and streaming failure extents agree"
    );
    let BankPlayBankDecision::Play(play) = fields.play_bank else {
        return Err("Play branch reached".into());
    };
    check_field(play.bank_id, 0, 102..106)?;
    assert!(
        play.bank_type.is_none(),
        "unreadable bank type has no receipt"
    );
    assert!(
        matches!(
            next(&mut hirc, &header)?.status,
            BankActionRead::Decoded { .. }
        ),
        "a malformed payload does not consume the following object"
    );
    Ok(())
}

#[test]
fn bank_source_action_rejects_equal_foreign_header_before_reading_the_object() -> TestResult {
    let bytes = TestBank::new(12).object(action(PLAY, 4)).bytes();
    let mut foreign = BankSource::open(Cursor::new(bytes.as_slice()), bytes.len() as u64)?;
    let (foreign_header, _) = header_and_hirc(&mut foreign)?;
    let trace = RefCell::new(Vec::new());
    let mut bank = traced_bank(&bytes, &trace, None)?;
    let (header, section) = header_and_hirc(&mut bank)?;
    assert_eq!(
        header.header(),
        foreign_header.header(),
        "both raw headers look identical"
    );
    let mut hirc = bank.hirc(&section)?;
    trace.borrow_mut().clear();
    assert!(
        matches!(
            hirc.next_action(&foreign_header),
            Some(Err(BankReadError::Format(_)))
        ),
        "a different bank read has no authority over this object"
    );
    assert!(
        trace.borrow().is_empty(),
        "no object reads before source binding succeeds"
    );
    assert_eq!(
        next(&mut hirc, &header)?.object.ordinal(),
        0,
        "same-owner retry sees original object"
    );
    Ok(())
}

#[test]
fn bank_source_action_keeps_selected_scalar_io_failure_as_a_hard_error() -> TestResult {
    let bytes = TestBank::new(12).object(action(PLAY, 4)).bytes();
    let trace = RefCell::new(Vec::new());
    let mut bank = traced_bank(&bytes, &trace, Some(BANK_TYPE_OFFSET))?;
    let (header, section) = header_and_hirc(&mut bank)?;
    let mut hirc = bank.hirc(&section)?;
    let Some(Err(BankReadError::Io(error))) = hirc.next_action(&header) else {
        return Err("physical read failure must not become a malformed Action row".into());
    };
    assert_eq!(
        error.kind(),
        io::ErrorKind::PermissionDenied,
        "original I/O error kind"
    );
    assert!(
        hirc.next_action(&header).is_none(),
        "physical failure ends the cursor"
    );
    Ok(())
}

fn check_non_play(status: BankActionRead) -> TestResult {
    let BankActionRead::Decoded { target, fields } = status else {
        return Err("second Action should decode".into());
    };
    assert_eq!(target.action_type, STOP, "original non-Play type");
    assert_eq!(
        target.bank, None,
        "non-Play ignores the decoy PlayBank bytes"
    );
    assert!(
        matches!(fields.play_bank, BankPlayBankDecision::NonPlay),
        "actual branch"
    );
    Ok(())
}

fn check_play_fields(status: BankActionRead) -> TestResult {
    let BankActionRead::Decoded { target, fields } = status else {
        return Err("version 154 Play must decode".into());
    };
    assert_eq!(
        (target.action_type, target.target_id),
        (PLAY, TARGET),
        "raw Action words"
    );
    assert_eq!(
        target.bank.map(|bank| (bank.bank_id, bank.bank_type)),
        Some((0, 0x9988_AABB)),
        "present zero bank ID and original unknown type"
    );
    check_field(fields.action_type, PLAY, 73..75)?;
    check_field(fields.target_id, TARGET, 75..79)?;
    let BankPlayBankDecision::Play(play) = fields.play_bank else {
        return Err("PlayBank branch receipt".into());
    };
    check_field(play.property_count, 2, 80..81)?;
    check_field(play.ranged_property_count, 1, 91..92)?;
    check_field(play.bank_id, 0, 102..106)?;
    check_field(play.bank_type, 0x9988_AABB, 106..110)
}

fn check_legacy_actions(bytes: &[u8]) -> TestResult {
    let bank = Soundbank::parse(bytes)?;
    let [first, second, _] = bank.objects() else {
        return Err("three original objects".into());
    };
    let (HircValue::Action(first), HircValue::Action(second)) = (&first.value, &second.value)
    else {
        return Err("legacy Action fields".into());
    };
    assert_eq!(
        (
            first.action_type,
            first.target_id,
            first.bank.map(|bank| (bank.bank_id, bank.bank_type))
        ),
        (PLAY, TARGET, Some((0, 0x9988_AABB))),
        "legacy Play values"
    );
    assert_eq!(
        (second.action_type, second.target_id, second.bank),
        (STOP, TARGET, None),
        "legacy non-Play values"
    );
    Ok(())
}

fn check_field<T: Eq + std::fmt::Debug>(
    field: Option<BankHircField<T>>,
    value: T,
    range: Range<u64>,
) -> TestResult {
    let field = field.ok_or("original scalar receipt")?;
    assert_eq!(
        (field.value, field.range),
        (value, range),
        "original value and exact bytes"
    );
    Ok(())
}

fn action(action_type: u16, bank_type_len: usize) -> TestHircObject {
    let mut fields = action_type.to_le_bytes().to_vec();
    fields.extend_from_slice(&TARGET.to_le_bytes());
    fields.extend_from_slice(&[0, 2]);
    fields.extend_from_slice(&[0xA1; 10]);
    fields.push(1);
    fields.extend_from_slice(&[0xB2; 9]);
    fields.push(0);
    fields.extend_from_slice(&0_u32.to_le_bytes());
    fields.extend_from_slice(0x9988_AABB_u32.to_le_bytes().split_at(bank_type_len).0);
    TestHircObject::raw(3, ACTION_ID, fields)
}

fn header_and_hirc<R: Read + Seek>(
    bank: &mut BankSource<R>,
) -> TestResult<(BankObservedHeader, BankSourceSection)> {
    let mut cursor = bank.section_cursor();
    let bkhd = bank.next_section(&mut cursor).ok_or("original BKHD")??;
    let hirc = bank.next_section(&mut cursor).ok_or("original HIRC")??;
    Ok((bank.read_observed_header_for(&bkhd)?, hirc))
}

fn next<R: Read + Seek>(
    hirc: &mut BankHirc<'_, R>,
    header: &BankObservedHeader,
) -> TestResult<BankHircAction> {
    Ok(hirc.next_action(header).ok_or("original HIRC object")??)
}

struct TraceReader<'a> {
    cursor: Cursor<&'a [u8]>,
    reads: &'a RefCell<Vec<Range<u64>>>,
    fault: Option<u64>,
}

impl Read for TraceReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let start = self.cursor.position();
        if self.fault == Some(start) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let count = self.cursor.read(output)?;
        self.reads.borrow_mut().push(start..start + count as u64);
        Ok(count)
    }
}

impl Seek for TraceReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(position)
    }
}

fn traced_bank<'a>(
    bytes: &'a [u8],
    reads: &'a RefCell<Vec<Range<u64>>>,
    fault: Option<u64>,
) -> Result<BankSource<TraceReader<'a>>, BankReadError> {
    let reader = TraceReader {
        cursor: Cursor::new(bytes),
        reads,
        fault,
    };
    BankSource::open(reader, bytes.len() as u64)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{HircValue, Soundbank};

use super::ActionTestResult;

pub(super) const PLAY: u16 = 0x0403;
pub(super) const STOP: u16 = 0x0102;
pub(super) const TARGET_ID: u32 = 0x5001;
const BANK_ID: u32 = 0x7a02;
const ACTION_ID: u32 = 0x6001;
const ACTION_TYPE_CODE: u8 = 3;
const SECTION_HEADER_BYTES: usize = 8;
const PROPERTY_BYTES: usize = 5;
const RANGED_PROPERTY_BYTES: usize = 9;

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) version: u32,
    pub(super) action_type: u16,
    pub(super) play: Play,
    pub(super) sites: Sites,
}

#[derive(Clone, Copy)]
pub(super) struct Play {
    pub(super) props: u8,
    pub(super) ranged: u8,
    pub(super) bank_id: u32,
    pub(super) bank_type: u32,
}

pub(super) struct Sites {
    pub(super) action_type: Range<usize>,
    pub(super) target_id: Range<usize>,
    pub(super) props: Range<usize>,
    pub(super) ranged: Range<usize>,
    pub(super) bank_id: Range<usize>,
    pub(super) bank_type: Range<usize>,
}

impl Fixture {
    pub(super) const fn selects_play_bank(&self) -> bool {
        self.version == 154 && self.action_type == PLAY
    }

    pub(super) fn verify_legacy_decoder(&self) -> ActionTestResult {
        let bank = Soundbank::parse(self.bytes.as_slice())?;
        let object = bank.objects().first().ok_or("fixture Action object")?;
        let HircValue::Action(action) = &object.value else {
            return Err("legacy decoder recognises the original Action fields".into());
        };
        assert_eq!(
            action.action_type, self.action_type,
            "original action type word"
        );
        assert_eq!(action.target_id, TARGET_ID, "original target word");
        let selected = action.bank.map(|bank| (bank.bank_id, bank.bank_type));
        assert_eq!(
            selected,
            self.selects_play_bank()
                .then_some((self.play.bank_id, self.play.bank_type)),
            "legacy parser supplies the same PlayBank decision and raw words"
        );
        Ok(())
    }
}

pub(super) fn fixture(version: u32, action_type: u16, play: Play) -> ActionTestResult<Fixture> {
    bank(version, action_type, play, true)
}

pub(super) fn without_bank_type(play: Play) -> ActionTestResult<Fixture> {
    bank(154, PLAY, play, false)
}

fn bank(version: u32, action_type: u16, play: Play, complete: bool) -> ActionTestResult<Fixture> {
    let mut bytes = Vec::new();
    let header = [version, BANK_ID, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    section(&mut bytes, *b"BKHD", &header)?;
    let hirc_start = bytes.len() + SECTION_HEADER_BYTES;
    let mut hirc = Vec::new();
    hirc.extend_from_slice(&1_u32.to_le_bytes());
    hirc.push(ACTION_TYPE_CODE);
    let size = hirc.len();
    hirc.extend_from_slice(&[0; 4]);
    let body = hirc.len();
    hirc.extend_from_slice(&ACTION_ID.to_le_bytes());
    let sites = action_fields(hirc_start, &mut hirc, action_type, play);
    if !complete {
        hirc.truncate(sites.bank_type.start - hirc_start);
    }
    let body_length = u32::try_from(hirc.len() - body)?.to_le_bytes();
    hirc.get_mut(size..body)
        .ok_or("fixture object size word")?
        .copy_from_slice(&body_length);
    section(&mut bytes, *b"HIRC", &hirc)?;
    Ok(Fixture {
        bytes,
        version,
        action_type,
        play,
        sites,
    })
}

fn action_fields(base: usize, hirc: &mut Vec<u8>, action_type: u16, play: Play) -> Sites {
    let action_type = write(base, hirc, &action_type.to_le_bytes());
    let target_id = write(base, hirc, &TARGET_ID.to_le_bytes());
    hirc.push(0);
    let props = write(base, hirc, &[play.props]);
    hirc.resize(hirc.len() + usize::from(play.props) * PROPERTY_BYTES, 0xb2);
    let ranged = write(base, hirc, &[play.ranged]);
    hirc.resize(
        hirc.len() + usize::from(play.ranged) * RANGED_PROPERTY_BYTES,
        0x95,
    );
    hirc.push(0);
    let bank_id = write(base, hirc, &play.bank_id.to_le_bytes());
    let bank_type = write(base, hirc, &play.bank_type.to_le_bytes());
    Sites {
        action_type,
        target_id,
        props,
        ranged,
        bank_id,
        bank_type,
    }
}

fn write(base: usize, bytes: &mut Vec<u8>, field: &[u8]) -> Range<usize> {
    let start = base + bytes.len();
    bytes.extend_from_slice(field);
    start..base + bytes.len()
}

fn section(bytes: &mut Vec<u8>, tag: [u8; 4], body: &[u8]) -> ActionTestResult {
    bytes.extend_from_slice(&tag);
    bytes.extend_from_slice(&u32::try_from(body.len())?.to_le_bytes());
    bytes.extend_from_slice(body);
    Ok(())
}

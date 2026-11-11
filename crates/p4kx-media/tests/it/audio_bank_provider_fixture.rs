// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::BankTestResult;

pub(super) const MEDIA_ID: u32 = 42;
pub(super) const OBJECT_ID: u32 = 99;
pub(super) const PREFETCH: &[u8] = b"RIFF\x50\0\0\0WAVE";
pub(super) const UNKNOWN_TAG: [u8; 4] = [b'X', 0xfe, b'Z', b'!'];
const VERSION: u32 = 154;

pub(super) struct BankFixture<'a> {
    pub(super) bytes: Vec<u8>,
    pub(super) sections: Vec<OriginalSection>,
    pub(super) media: [&'a [u8]; 2],
}

pub(super) struct OriginalSection {
    pub(super) tag: [u8; 4],
    pub(super) header: Range<usize>,
    pub(super) body: Range<usize>,
}

pub(super) fn fixture(
    bank_id: u32,
    language_id: u32,
    first_media: &[u8],
    duplicate_data: bool,
) -> BankTestResult<BankFixture<'_>> {
    let mut fixture = BankFixture {
        bytes: Vec::new(),
        sections: Vec::new(),
        media: [first_media, PREFETCH],
    };
    let header = words([VERSION, bank_id, language_id]);
    fixture.append(*b"BKHD", &header)?;
    let first_size = u32::try_from(first_media.len())?;
    let prefetch_size = u32::try_from(PREFETCH.len())?;
    fixture.append(
        *b"DIDX",
        &words([MEDIA_ID, 0, first_size, MEDIA_ID, first_size, prefetch_size]),
    )?;
    fixture.append(*b"HIRC", &hierarchy())?;
    fixture.append(UNKNOWN_TAG, b"opaque")?;
    let data = [first_media, PREFETCH].concat();
    fixture.append(*b"DATA", &data)?;
    fixture.append(UNKNOWN_TAG, b"repeated opaque")?;
    if duplicate_data {
        fixture.append(*b"DATA", b"other actual DATA")?;
    }
    Ok(fixture)
}

impl BankFixture<'_> {
    fn append(&mut self, tag: [u8; 4], body: &[u8]) -> BankTestResult {
        let header = self.bytes.len();
        self.bytes.extend_from_slice(&tag);
        self.bytes
            .extend_from_slice(&u32::try_from(body.len())?.to_le_bytes());
        let start = self.bytes.len();
        self.bytes.extend_from_slice(body);
        self.sections.push(OriginalSection {
            tag,
            header: header..start,
            body: start..self.bytes.len(),
        });
        Ok(())
    }
}

fn hierarchy() -> Vec<u8> {
    let mut body = words([2]);
    body.push(4);
    body.extend_from_slice(&words([5, OBJECT_ID]));
    body.push(0);
    body.push(0xe2);
    body.extend_from_slice(&words([6, OBJECT_ID]));
    body.extend_from_slice(&[0xc2, 0xd3, 0x99, 0x88]);
    body
}

fn words<const N: usize>(values: [u32; N]) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

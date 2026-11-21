// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::ops::Range;

use p4kx_extraction::{CancelToken, Guid};

/// The suffix of the string before a provider preset GUID.
const PRESET_KEY: &[u8] = b"preset";

const GUID_LEN: usize = 36;

const _: () = assert!(
    PRESET_KEY.len() <= GUID_LEN,
    "the tail must hold the whole preset key"
);

const CHUNK_LEN: usize = 64 << 10;

/// The first matched GUID and the half-open decoded byte ranges of its two tokens.
#[derive(Debug)]
pub(crate) struct PresetMatch {
    pub(crate) guid: Guid,
    pub(crate) key: Range<u64>,
    pub(crate) value: Range<u64>,
}

#[derive(Clone, Copy)]
enum Completion {
    FirstMatch,
    EndOfEntry,
}

/// Scans the null-separated SOC strings using a bounded token tail.
#[derive(Default)]
pub(crate) struct PresetScan {
    tail: Vec<u8>,
    offset: u64,
    token_start: u64,
    previous_key: Option<Range<u64>>,
}

impl PresetScan {
    /// Returns the first matching GUID or reads to the end of the stream.
    pub(crate) fn read(
        reader: impl Read,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Option<Guid>> {
        let found = Self::scan(reader, cancel, Completion::FirstMatch)?;
        Ok(found.map(|matched| matched.guid))
    }

    /// Retains the first match and completes the same stream to its end.
    pub(crate) fn read_to_end(
        reader: impl Read,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Option<PresetMatch>> {
        Self::scan(reader, cancel, Completion::EndOfEntry)
    }

    fn scan(
        mut reader: impl Read,
        cancel: Option<&CancelToken>,
        completion: Completion,
    ) -> io::Result<Option<PresetMatch>> {
        let mut scan = Self::default();
        let mut chunk = vec![0; CHUNK_LEN];
        let mut first = None;
        loop {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let read = reader.read(&mut chunk)?;
            if read == 0 {
                return Ok(first.or_else(|| scan.finish_token()));
            }
            if first.is_none() {
                let piece = chunk
                    .get(..read)
                    .ok_or_else(|| io::Error::other("invalid SOC read length"))?;
                first = scan.feed(piece)?;
                if first.is_some() && matches!(completion, Completion::FirstMatch) {
                    return Ok(first);
                }
            }
        }
    }

    fn feed(&mut self, mut piece: &[u8]) -> io::Result<Option<PresetMatch>> {
        while let Some(zero) = piece.iter().position(|&byte| byte == 0) {
            let (token, separator) = piece.split_at(zero);
            self.extend(token)?;
            let found = self.finish_token();
            self.advance(1)?;
            self.token_start = self.offset;
            if found.is_some() {
                return Ok(found);
            }
            piece = separator.get(1..).unwrap_or_default();
        }
        self.extend(piece)?;
        Ok(None)
    }

    fn advance(&mut self, bytes: usize) -> io::Result<()> {
        self.offset = u64::try_from(bytes)
            .ok()
            .and_then(|bytes| self.offset.checked_add(bytes))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "SOC offset overflow"))?;
        Ok(())
    }

    fn extend(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.advance(bytes.len())?;
        if let Some(last) = bytes.last_chunk::<GUID_LEN>() {
            self.tail.clear();
            self.tail.extend_from_slice(last);
        } else {
            let retained = GUID_LEN - bytes.len();
            let discard = self.tail.len().saturating_sub(retained);
            self.tail.drain(..discard);
            self.tail.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn finish_token(&mut self) -> Option<PresetMatch> {
        let start = self.token_start;
        let end = self.offset;
        let previous_key = self.previous_key.take();
        self.previous_key = self.tail.ends_with(PRESET_KEY).then_some(start..end);
        let found = previous_key.and_then(|key| {
            if end - start != GUID_LEN as u64 {
                return None;
            }
            let guid = std::str::from_utf8(&self.tail).ok()?.parse().ok()?;
            Some(PresetMatch {
                guid,
                key,
                value: start..end,
            })
        });
        self.tail.clear();
        found
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use p4kx_extraction::{CancelToken, Guid};

    use super::PresetScan;

    const PRESET: &str = "1f712a22-c9d5-438e-8605-fac7651e30fc";

    fn guid(text: &str) -> Guid {
        text.parse().expect("a GUID")
    }

    fn scan(bytes: &[u8]) -> Option<Guid> {
        PresetScan::read(bytes, None).expect("a slice reads")
    }

    #[test]
    fn the_guid_after_the_preset_key_is_found() {
        let bytes = [b"x\0".as_slice(), b"preset\0", PRESET.as_bytes(), b"\0y"].concat();
        assert_eq!(scan(&bytes), Some(guid(PRESET)));
    }

    #[test]
    fn the_last_string_of_the_file_needs_no_terminator() {
        let bytes = [b"preset\0".as_slice(), PRESET.as_bytes()].concat();
        assert_eq!(scan(&bytes), Some(guid(PRESET)));
    }

    #[test]
    fn a_key_that_merely_ends_in_preset_counts() {
        let bytes = [b"harvestpreset\0".as_slice(), PRESET.as_bytes(), b"\0"].concat();
        assert_eq!(scan(&bytes), Some(guid(PRESET)));
    }

    #[test]
    fn the_first_preset_wins() {
        let other = "2f712a22-c9d5-438e-8605-fac7651e30fc";
        let bytes = [
            b"preset\0".as_slice(),
            PRESET.as_bytes(),
            b"\0preset\0",
            other.as_bytes(),
            b"\0",
        ]
        .concat();
        assert_eq!(scan(&bytes), Some(guid(PRESET)));
    }

    #[test]
    fn nothing_that_is_not_a_guid_after_the_key_is_found() {
        for bytes in [
            b"preset\0not a guid\0".as_slice(),
            b"preset\0",
            b"preset",
            b"",
            b"\0\0\0",
            b"1f712a22-c9d5-438e-8605-fac7651e30fc\0",
            b"other\x001f712a22-c9d5-438e-8605-fac7651e30fc\0",
            b"preset\0x1f712a22-c9d5-438e-8605-fac7651e30fc\0",
            b"preset\0preset\0",
        ] {
            assert_eq!(scan(bytes), None, "{bytes:?}");
        }
    }

    #[test]
    fn token_byte_ranges_survive_every_chunk_boundary() -> io::Result<()> {
        let bytes = [b"x\0harvestpreset\0".as_slice(), PRESET.as_bytes(), b"\0"].concat();
        for cut in 0..=bytes.len() {
            let (first, second) = bytes.split_at(cut);
            let mut scan = PresetScan::default();
            let found = match scan.feed(first)? {
                Some(found) => Some(found),
                None => scan.feed(second)?,
            }
            .ok_or_else(|| io::Error::other("preset expected"))?;
            assert_eq!(found.guid, guid(PRESET), "cut at {cut}");
            assert_eq!(found.key, 2..15, "whole original key at cut {cut}");
            assert_eq!(found.value, 16..52, "whole GUID at cut {cut}");
        }
        Ok(())
    }

    #[test]
    fn completion_preserves_first_match_and_accepts_an_unterminated_token() -> io::Result<()> {
        let only = [b"preset\0".as_slice(), PRESET.as_bytes()].concat();
        let found = PresetScan::read_to_end(only.as_slice(), None)?
            .ok_or_else(|| io::Error::other("preset at EOF expected"))?;
        assert_eq!(found.value, 7..43, "EOF supplies the token terminator");
        let other = b"2f712a22-c9d5-438e-8605-fac7651e30fc";
        let both = [only.as_slice(), b"\0preset\0", other, b"\0"].concat();
        let first = PresetScan::read_to_end(both.as_slice(), None)?
            .ok_or_else(|| io::Error::other("first preset expected"))?;
        assert_eq!(
            first.guid,
            guid(PRESET),
            "EOF checking retains the first match"
        );
        assert_eq!(first.key, found.key, "same key bytes");
        assert_eq!(first.value, found.value, "same value bytes");
        Ok(())
    }

    #[test]
    fn a_preset_after_a_256_mib_prefix_is_read() -> io::Result<()> {
        let prefix = io::repeat(b'x').take(256 << 20);
        let suffix = [b"\0preset\0".as_slice(), PRESET.as_bytes(), b"\0"].concat();
        let reader = prefix.chain(suffix.as_slice());
        assert_eq!(PresetScan::read(reader, None)?, Some(guid(PRESET)));
        Ok(())
    }

    struct CancelDuringRead<'a> {
        cancel: &'a CancelToken,
        bytes: &'a [u8],
    }

    impl Read for CancelDuringRead<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            assert!(
                !self.cancel.is_cancelled(),
                "no extra read after cancellation"
            );
            let read = self.bytes.read(buffer)?;
            self.cancel.cancel();
            Ok(read)
        }
    }

    #[test]
    fn a_cancelled_stream_stops_between_chunks() {
        let cancel = CancelToken::new();
        let result = PresetScan::read(
            CancelDuringRead {
                cancel: &cancel,
                bytes: b"x",
            },
            Some(&cancel),
        );
        assert_eq!(
            result.expect_err("cancelled scan").kind(),
            io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn a_string_longer_than_a_guid_is_no_preset() {
        let long = format!("preset\0{PRESET}0\0");
        assert_eq!(scan(long.as_bytes()), None);
    }

    #[test]
    fn verified_completion_cancels_after_the_first_match() {
        let bytes = [b"preset\0".as_slice(), PRESET.as_bytes(), b"\0"].concat();
        let cancel = CancelToken::new();
        let result = PresetScan::read_to_end(
            CancelDuringRead {
                cancel: &cancel,
                bytes: &bytes,
            },
            Some(&cancel),
        );
        assert_eq!(
            result.expect_err("cancellation prevents completion").kind(),
            io::ErrorKind::Interrupted,
            "the matching GUID is withheld when the verified read cannot finish"
        );
    }
}

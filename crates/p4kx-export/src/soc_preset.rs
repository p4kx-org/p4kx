// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_extraction::{CancelToken, Guid};

/// The string a container's `.soc` stores right before the GUID of its provider preset.
const PRESET_KEY: &[u8] = b"preset";

/// The length of a GUID in text form.
const GUID_LEN: usize = 36;

/// The tail kept of a string holds the last [`GUID_LEN`] bytes, which must also hold the whole of
/// [`PRESET_KEY`] for the test of its end.
const _: () = assert!(
    PRESET_KEY.len() <= GUID_LEN,
    "the tail must hold the whole preset key"
);

/// The size of the pieces a `.soc` is read in.
const CHUNK_LEN: usize = 64 << 10;

/// Finds the GUID that follows the first string ending in `preset` in a `.soc` file, which stores
/// its strings separated by a zero byte. The file is fed piece by piece and never held whole.
#[derive(Default)]
pub(crate) struct PresetScan {
    tail: Vec<u8>,
    length: usize,
    after_key: bool,
}

impl PresetScan {
    /// Reads to the first preset or the end of the stream.
    ///
    /// # Errors
    ///
    /// Returns a read failure or [`io::ErrorKind::Interrupted`] on cancellation.
    pub(crate) fn read(
        mut reader: impl Read,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Option<Guid>> {
        let mut scan = Self::default();
        let mut chunk = vec![0; CHUNK_LEN];
        loop {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let read = reader.read(&mut chunk)?;
            if read == 0 {
                return Ok(scan.finish());
            }
            if let Some(guid) = chunk.get(..read).and_then(|piece| scan.feed(piece)) {
                return Ok(Some(guid));
            }
        }
    }

    /// Scans the next piece of the file and returns the preset once its GUID is complete.
    pub(crate) fn feed(&mut self, piece: &[u8]) -> Option<Guid> {
        let mut rest = piece;
        while let Some(zero) = rest.iter().position(|&byte| byte == 0) {
            let (string, after) = rest.split_at(zero);
            self.extend(string);
            if let Some(guid) = self.finish() {
                return Some(guid);
            }
            rest = after.get(1..).unwrap_or_default();
        }
        self.extend(rest);
        None
    }

    fn extend(&mut self, bytes: &[u8]) {
        self.length = self.length.saturating_add(bytes.len());
        self.tail.extend_from_slice(bytes);
        if let Some(surplus) = self.tail.len().checked_sub(GUID_LEN) {
            self.tail.drain(..surplus);
        }
    }

    fn finish(&mut self) -> Option<Guid> {
        let found = (self.after_key && self.length == GUID_LEN)
            .then(|| std::str::from_utf8(&self.tail).ok()?.parse::<Guid>().ok())
            .flatten();
        self.after_key = self.tail.ends_with(PRESET_KEY);
        self.tail.clear();
        self.length = 0;
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
    fn a_guid_cut_between_pieces_is_found() {
        let bytes = [b"preset\0".as_slice(), PRESET.as_bytes(), b"\0"].concat();
        for cut in 0..bytes.len() {
            let (first, second) = bytes.split_at(cut);
            let mut scan = PresetScan::default();
            let found = scan.feed(first).or_else(|| scan.feed(second));
            assert_eq!(found, Some(guid(PRESET)), "cut at {cut}");
        }
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
    }

    impl Read for CancelDuringRead<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            assert!(
                !self.cancel.is_cancelled(),
                "no extra read after cancellation"
            );
            self.cancel.cancel();
            let Some(first) = buffer.first_mut() else {
                return Ok(0);
            };
            *first = b'x';
            Ok(1)
        }
    }

    #[test]
    fn a_cancelled_stream_stops_between_chunks() {
        let cancel = CancelToken::new();
        let result = PresetScan::read(CancelDuringRead { cancel: &cancel }, Some(&cancel));
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
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};

use p4kx_extraction::{
    ArchiveByteRange, ArchiveRegionState, ArchiveTableDeclaration, ArchiveZip64End,
    ArchiveZipLayout,
};

use super::TestResult;
use super::layout_inventory::require;

const CLASSIC_LEN: usize = 22;
const LOCATOR_LEN: usize = 20;
const ZIP64_LEN: usize = 56;
const INSTALL_COMMENT_LEN: usize = 16;

pub(super) const PROGRESS_LEN: usize = 12;
pub(super) const FREE_LEN: usize = 16;

pub(super) struct Footer {
    classic_at: u64,
    classic: [u8; CLASSIC_LEN],
    locator_at: u64,
    locator: [u8; LOCATOR_LEN],
    zip64_at: u64,
    zip64: [u8; ZIP64_LEN],
    pub(super) installation: Option<Installation>,
}

pub(super) struct Installation {
    sector_size: u16,
    pub(super) free_count: u64,
    pub(super) progress_at: u64,
    pub(super) free_at: u64,
}

impl Footer {
    pub(super) fn read(file: &mut File, source_len: u64) -> TestResult<Self> {
        let tail_len =
            usize::try_from(source_len.min((CLASSIC_LEN + usize::from(u16::MAX)) as u64))?;
        let tail_at = source_len - tail_len as u64;
        let mut tail = vec![0; tail_len];
        read_at(file, tail_at, &mut tail)?;
        let (index, classic) = tail
            .windows(CLASSIC_LEN)
            .enumerate()
            .rev()
            .find(|(_, bytes)| bytes.starts_with(b"PK\x05\x06"))
            .ok_or("classic ZIP end signature")?;
        let classic = classic.try_into()?;
        let classic_at = tail_at + u64::try_from(index)?;
        let locator_at = classic_at
            .checked_sub(LOCATOR_LEN as u64)
            .ok_or("locator position")?;
        let locator = fixed_at::<LOCATOR_LEN>(file, locator_at)?;
        require(
            locator.starts_with(b"PK\x06\x07"),
            "adjacent ZIP64 locator signature",
        )?;
        let zip64_at = word::<8>(&locator, 8).map(u64::from_le_bytes)?;
        let zip64 = fixed_at::<ZIP64_LEN>(file, zip64_at)?;
        require(zip64.starts_with(b"PK\x06\x06"), "ZIP64 end signature")?;
        let mut result = Self {
            classic_at,
            classic,
            locator_at,
            locator,
            zip64_at,
            zip64,
            installation: None,
        };
        result.installation = result.read_installation(file)?;
        Ok(result)
    }

    pub(super) fn directory_offset(&self) -> TestResult<u64> {
        Ok(u64::from_le_bytes(word(&self.zip64, 48)?))
    }

    pub(super) fn directory_count(&self) -> TestResult<u64> {
        Ok(u64::from_le_bytes(word(&self.zip64, 32)?))
    }

    fn read_installation(&self, file: &mut File) -> TestResult<Option<Installation>> {
        if usize::from(u16::from_le_bytes(word(&self.classic, 20)?)) != INSTALL_COMMENT_LEN {
            return Ok(None);
        }
        let comment = fixed_at::<INSTALL_COMMENT_LEN>(file, self.classic_at + CLASSIC_LEN as u64)?;
        if !comment.starts_with(b"CIG\0\x01\0") {
            return Ok(None);
        }
        let sector_size = u16::from_le_bytes(word(&comment, 6)?);
        let free_count = u64::from_le_bytes(word(&comment, 8)?);
        let free_len = free_count
            .checked_mul(FREE_LEN as u64)
            .ok_or("free table length")?;
        let progress_len = self
            .directory_count()?
            .checked_mul(PROGRESS_LEN as u64)
            .ok_or("progress table length")?;
        let free_at = self
            .directory_offset()?
            .checked_sub(free_len)
            .ok_or("free table offset")?;
        let progress_at = free_at
            .checked_sub(progress_len)
            .ok_or("progress table offset")?;
        Ok(Some(Installation {
            sector_size,
            free_count,
            progress_at,
            free_at,
        }))
    }

    pub(super) fn verify(&self, layout: &ArchiveZipLayout, source_len: u64) -> TestResult {
        self.verify_classic(layout, source_len)?;
        let locator = layout.locator.as_ref().ok_or("ZIP64 locator projection")?;
        require(
            same_range(locator.range, self.locator_at, LOCATOR_LEN as u64),
            "ZIP64 locator extent",
        )?;
        require(
            locator.disk == u32::from_le_bytes(word(&self.locator, 4)?),
            "ZIP64 locator disk",
        )?;
        require(
            locator.target.get() == self.zip64_at,
            "ZIP64 locator target",
        )?;
        require(
            locator.disks == u32::from_le_bytes(word(&self.locator, 16)?),
            "ZIP64 locator disk count",
        )?;
        self.verify_zip64(layout)?;
        self.verify_installation(layout)?;
        Ok(())
    }

    fn verify_classic(&self, layout: &ArchiveZipLayout, source_len: u64) -> TestResult {
        let end = &layout.end;
        require(
            same_range(end.fixed, self.classic_at, CLASSIC_LEN as u64),
            "classic end extent",
        )?;
        for (actual, offset) in [
            (end.disk, 4),
            (end.directory_disk, 6),
            (end.entries_on_disk, 8),
            (end.entries, 10),
            (end.comment_length, 20),
        ] {
            require(
                actual == u16::from_le_bytes(word(&self.classic, offset)?),
                "classic end 16-bit word",
            )?;
        }
        require(
            end.directory_size == u32::from_le_bytes(word(&self.classic, 12)?),
            "classic directory size",
        )?;
        require(
            end.directory_offset == u32::from_le_bytes(word(&self.classic, 16)?),
            "classic directory offset",
        )?;
        let comment_at = self.classic_at + CLASSIC_LEN as u64;
        let comment_len = u64::from(end.comment_length);
        let following_at = comment_at + comment_len;
        require(
            end.comment.offset.get() == comment_at
                && end.comment.declared_length.get() == comment_len,
            "classic comment extent",
        )?;
        require(
            matches!(end.comment.state, ArchiveRegionState::Complete),
            "classic comment state",
        )?;
        require(
            end.comment
                .available
                .is_some_and(|range| same_range(range, comment_at, comment_len)),
            "classic comment available extent",
        )?;
        let following_len = source_len
            .checked_sub(following_at)
            .ok_or("ZIP comment bounds")?;
        require(
            end.following
                .is_some_and(|range| same_range(range, following_at, following_len)),
            "bytes following classic comment",
        )?;
        Ok(())
    }

    fn verify_zip64(&self, layout: &ArchiveZipLayout) -> TestResult {
        let end = layout.zip64.as_ref().ok_or("ZIP64 end projection")?;
        require(
            same_range(end.fixed, self.zip64_at, ZIP64_LEN as u64),
            "ZIP64 end extent",
        )?;
        for (actual, offset) in [(end.version_made_by, 12), (end.version_needed, 14)] {
            require(
                actual == u16::from_le_bytes(word(&self.zip64, offset)?),
                "ZIP64 end 16-bit word",
            )?;
        }
        for (actual, offset) in [(end.disk, 16), (end.directory_disk, 20)] {
            require(
                actual == u32::from_le_bytes(word(&self.zip64, offset)?),
                "ZIP64 end 32-bit word",
            )?;
        }
        for (actual, offset) in [
            (end.declared_size, 4),
            (end.entries_on_disk, 24),
            (end.entries, 32),
            (end.directory_size, 40),
            (end.directory_offset, 48),
        ] {
            require(
                actual.get() == u64::from_le_bytes(word(&self.zip64, offset)?),
                "ZIP64 end 64-bit word",
            )?;
        }
        require(
            layout.count.get() == self.directory_count()?,
            "original directory record count",
        )?;
        require(
            same_range(
                layout.records,
                self.directory_offset()?,
                end.directory_size.get(),
            ),
            "original directory extent",
        )?;
        self.verify_extension(end)
    }

    fn verify_extension(&self, end: &ArchiveZip64End) -> TestResult {
        let extension = end
            .extension
            .as_ref()
            .ok_or("ZIP64 extension declaration")?;
        let length = end
            .declared_size
            .get()
            .checked_sub((ZIP64_LEN - 12) as u64)
            .ok_or("ZIP64 end size")?;
        let offset = self.zip64_at + ZIP64_LEN as u64;
        require(
            extension.offset.get() == offset && extension.declared_length.get() == length,
            "ZIP64 extension extent",
        )?;
        require(
            matches!(extension.state, ArchiveRegionState::Complete),
            "ZIP64 extension state",
        )?;
        require(
            extension
                .available
                .is_some_and(|range| same_range(range, offset, length)),
            "ZIP64 extension available extent",
        )?;
        Ok(())
    }

    fn verify_installation(&self, layout: &ArchiveZipLayout) -> TestResult {
        match (&self.installation, &layout.installation) {
            (Some(expected), Some(actual)) => {
                require(
                    actual.sector_size == expected.sector_size
                        && actual.free_count.get() == expected.free_count,
                    "CIG sector size and free count",
                )?;
                let ArchiveTableDeclaration::Located { progress, free } = actual.tables else {
                    return Err("located CIG tables".into());
                };
                require(
                    same_range(
                        progress,
                        expected.progress_at,
                        expected.free_at - expected.progress_at,
                    ),
                    "CIG progress table extent",
                )?;
                require(
                    same_range(
                        free,
                        expected.free_at,
                        self.directory_offset()? - expected.free_at,
                    ),
                    "CIG free table extent",
                )?;
            }
            (None, None) => {}
            _ => return Err("CIG declaration presence".into()),
        }
        Ok(())
    }
}

const fn same_range(range: ArchiveByteRange, offset: u64, length: u64) -> bool {
    range.offset.get() == offset && range.length.get() == length
}

pub(super) fn read_at(file: &mut File, offset: u64, bytes: &mut [u8]) -> TestResult {
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(bytes)?;
    Ok(())
}

pub(super) fn fixed_at<const N: usize>(file: &mut File, offset: u64) -> TestResult<[u8; N]> {
    let mut bytes = [0; N];
    read_at(file, offset, &mut bytes)?;
    Ok(bytes)
}

pub(super) fn word<const N: usize>(bytes: &[u8], start: usize) -> TestResult<[u8; N]> {
    Ok(bytes
        .get(start..start + N)
        .ok_or("source word bounds")?
        .try_into()?)
}

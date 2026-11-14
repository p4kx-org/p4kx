// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;

use p4kx_texture::{LocatedMip, SiblingReader, read_mip_with_location};

use crate::disk_parts::DiskParts;
use crate::output_file::read_error;
use crate::{ConversionError, FileStamp, ObservedFileOrigin};

pub(super) struct DiskMipRead {
    pub(super) mip: LocatedMip,
    pub(super) header: Option<ObservedFileOrigin>,
    pub(super) part: Option<ObservedFileOrigin>,
}

pub(super) fn read(
    parts: &DiskParts,
    header_length: u64,
    level: u32,
    name: &str,
) -> Result<DiskMipRead, ConversionError> {
    let (path, file) = parts
        .open_with_path(None)
        .map_err(|source| read_error(parts.header(), source))?;
    let mut header = DiskRead::new(path, file);
    let siblings = ObservedSiblings {
        parts,
        observed: RefCell::new(None),
    };
    let mip = read_mip_with_location(&mut header, header_length, &siblings, level)
        .map_err(|error| ConversionError::of_texture(name, error))?;
    Ok(DiskMipRead {
        mip,
        header: header.origin(),
        part: siblings.observed.into_inner(),
    })
}

pub(super) struct DiskRead {
    path: PathBuf,
    file: File,
    before: Option<FileStamp>,
    extent: u64,
}

impl DiskRead {
    pub(super) fn new(path: PathBuf, file: File) -> Self {
        Self {
            path,
            before: file
                .metadata()
                .ok()
                .map(|meta| FileStamp::from_metadata(&meta)),
            file,
            extent: 0,
        }
    }

    pub(super) fn origin(&self) -> Option<ObservedFileOrigin> {
        ObservedFileOrigin::of_extent(&self.path, &self.file, self.before?, self.extent)
            .ok()
            .flatten()
    }
}

impl Read for DiskRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read(buffer)?;
        self.extent = self
            .extent
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("observed texture byte range overflows"))?;
        Ok(count)
    }
}

struct ObservedSiblings<'parts> {
    parts: &'parts DiskParts,
    observed: RefCell<Option<ObservedFileOrigin>>,
}

impl SiblingReader for ObservedSiblings<'_> {
    type Part<'read>
        = ObservedSibling<'read>
    where
        Self: 'read;

    fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        self.parts.part_len(index)
    }

    fn open_part(&self, index: u32) -> io::Result<Self::Part<'_>> {
        let (path, file) = self.parts.open_with_path(Some(index))?;
        Ok(ObservedSibling {
            reader: DiskRead::new(path, file),
            observed: &self.observed,
        })
    }
}

struct ObservedSibling<'read> {
    reader: DiskRead,
    observed: &'read RefCell<Option<ObservedFileOrigin>>,
}

impl Read for ObservedSibling<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buffer)
    }
}

impl Drop for ObservedSibling<'_> {
    fn drop(&mut self) {
        *self.observed.borrow_mut() = self.reader.origin();
    }
}

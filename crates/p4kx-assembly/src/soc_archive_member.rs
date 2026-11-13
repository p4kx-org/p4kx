// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_extraction::{Archive, ArchiveError, Entry, EntryOrigin, PathIndex};
use p4kx_extraction::{PathIndexBuildError, PathIndexError};
use thiserror::Error;

/// A physical SOC input with its verified payload or the original read failure.
#[derive(Debug)]
pub struct SocArchiveMember {
    origin: EntryOrigin,
    payload: Result<Vec<u8>, SocMemberError>,
}

impl SocArchiveMember {
    pub(super) fn read(
        archive: &Archive,
        entry: &Entry,
        check: impl FnMut() -> Result<(), super::SocCaptureError>,
    ) -> Result<Self, super::SocCaptureError> {
        let origin = origin(archive, entry)?;
        let payload = read_payload(archive, entry, check)?;
        Ok(Self { origin, payload })
    }

    /// Returns the admitted original Entry and containing context.
    #[must_use]
    pub const fn origin(&self) -> &EntryOrigin {
        &self.origin
    }

    /// Returns the verified bytes, or the failure at the admitted input.
    ///
    /// # Errors
    /// Returns the Archive decoder or byte-read error.
    pub fn payload(&self) -> Result<&[u8], &SocMemberError> {
        self.payload.as_deref()
    }
}

/// The reason an admitted SOC payload could not be read.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocMemberError {
    /// The Archive could not open the selected Entry.
    #[error(transparent)]
    Open(#[from] ArchiveError),
    /// Reading the complete decoded payload failed.
    #[error(transparent)]
    Read(#[from] io::Error),
}

fn read_payload(
    archive: &Archive,
    entry: &Entry,
    mut check: impl FnMut() -> Result<(), super::SocCaptureError>,
) -> Result<Result<Vec<u8>, SocMemberError>, super::SocCaptureError> {
    let mut reader = match archive.open_entry_verified(entry) {
        Ok(reader) => reader.reject_overlong(),
        Err(error) => return Ok(Err(error.into())),
    };
    let mut bytes = Vec::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        check()?;
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(Ok(bytes)),
            Ok(count) => {
                bytes.try_reserve(count)?;
                bytes.extend_from_slice(
                    buffer
                        .get(..count)
                        .ok_or(super::SocCaptureError::ReadWidth)?,
                );
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Ok(Err(error.into())),
        }
    }
}

pub(super) fn origin(
    archive: &Archive,
    entry: &Entry,
) -> Result<EntryOrigin, super::SocCaptureError> {
    archive
        .entry_origin(entry)
        .ok_or(super::SocCaptureError::Origin)
}

pub(super) fn select<'archive>(
    index: &PathIndex<'archive>,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
    mut check: impl FnMut() -> Result<(), super::SocCaptureError>,
) -> Result<Vec<&'archive Entry>, super::SocCaptureError> {
    for path in paths {
        let found = index
            .matching_entries_with(path.as_ref(), |_| check())
            .map_err(path_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(path_error)?;
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Ok(Vec::new())
}

pub(super) fn path_error(
    error: PathIndexBuildError<super::SocCaptureError>,
) -> super::SocCaptureError {
    match error {
        PathIndexBuildError::Interrupted(error) => error,
        PathIndexBuildError::Index(error) => super::SocCaptureError::Index(error),
        _ => super::SocCaptureError::Index(PathIndexError::SourceMismatch),
    }
}

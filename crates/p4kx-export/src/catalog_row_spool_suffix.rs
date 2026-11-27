// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{Seek as _, SeekFrom};
use std::num::NonZeroUsize;
use std::sync::Arc;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::{
    RowIndex, RowSpool, RowSpoolError, RowSpoolWriter, check_backing_length, checkpoint,
    poisoned_lock,
};
use crate::CatalogError;

/// Appends an ordered frame to an already-open spool when the first row arrives.
pub(crate) struct RowSpoolSuffixWriter<'file> {
    file: Option<&'file mut File>,
    writer: Option<RowSpoolWriter<&'file mut File>>,
}

impl<'file> RowSpoolSuffixWriter<'file> {
    const fn new(file: &'file mut File) -> Self {
        Self {
            file: Some(file),
            writer: None,
        }
    }

    pub(crate) fn append(
        &mut self,
        row: &impl Serialize,
        cancel: Option<&CancelToken>,
    ) -> Result<(), RowSpoolError> {
        let mut writer = if let Some(writer) = self.writer.take() {
            writer
        } else {
            let file = self.file.take().ok_or(RowSpoolError::Closed)?;
            checkpoint(cancel)?;
            RowSpoolWriter::with_output(file)?
        };
        writer.append(row, cancel)?;
        self.writer = Some(writer);
        Ok(())
    }

    fn finish(
        self,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Vec<NonZeroUsize>>, RowSpoolError> {
        checkpoint(cancel)?;
        if let Some(writer) = self.writer {
            let (_, lengths) = writer.finish_output(cancel)?;
            Ok(Some(lengths))
        } else if self.file.is_some() {
            Ok(None)
        } else {
            Err(RowSpoolError::Closed)
        }
    }
}

impl RowSpool {
    /// Seals a second frame on the same backing before either view is published.
    pub(crate) fn append_suffix<T>(
        &mut self,
        cancel: Option<&CancelToken>,
        project: impl FnOnce(&mut RowSpoolSuffixWriter<'_>) -> Result<T, CatalogError>,
    ) -> Result<(T, Option<Self>), CatalogError> {
        checkpoint(cancel)?;
        let backing = Arc::get_mut(&mut self.backing).ok_or(RowSpoolError::Corrupt(
            "cannot append to a shared spool backing",
        ))?;
        let file = backing.file.get_mut().map_err(|_| poisoned_lock())?;
        let offset = backing.bytes;
        check_backing_length(file, offset)?;
        file.seek(SeekFrom::Start(offset))
            .map_err(RowSpoolError::Io)?;
        let (result, frame) = match append_frame(file, offset, cancel, project) {
            Ok(staged) => staged,
            Err(primary) => {
                if let Err(rollback) = file.set_len(offset) {
                    return Err(RowSpoolError::SuffixRollback {
                        primary: Box::new(primary),
                        rollback,
                    }
                    .into());
                }
                return Err(primary);
            }
        };
        let Some(frame) = frame else {
            return Ok((result, None));
        };
        backing.bytes = frame.end;
        Ok((
            result,
            Some(Self {
                backing: Arc::clone(&self.backing),
                offset,
                bytes: frame.end - offset,
                index: RowIndex::Sequential(frame.lengths),
            }),
        ))
    }
}

struct SuffixFrame {
    end: u64,
    lengths: Vec<NonZeroUsize>,
}

fn append_frame<T>(
    file: &mut File,
    offset: u64,
    cancel: Option<&CancelToken>,
    project: impl FnOnce(&mut RowSpoolSuffixWriter<'_>) -> Result<T, CatalogError>,
) -> Result<(T, Option<SuffixFrame>), CatalogError> {
    let mut writer = RowSpoolSuffixWriter::new(file);
    let result = project(&mut writer)?;
    let Some(lengths) = writer.finish(cancel)? else {
        return Ok((result, None));
    };
    let end = file.metadata().map_err(RowSpoolError::Io)?.len();
    if end <= offset {
        return Err(RowSpoolError::Corrupt("appended frame has no compressed bytes").into());
    }
    Ok((result, Some(SuffixFrame { end, lengths })))
}

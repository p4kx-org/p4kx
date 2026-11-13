// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::Path;

use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};
use crate::error;
use crate::input_budget::InputBudget;
use crate::{Limits, MediaError, MediaWriter, SourceContext};

pub(crate) fn read_all(
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Vec<u8>, MediaError> {
    let budget = &context.directory.input_state()?.budget;
    let limits = writer.limits();
    let size = context.source.size;
    let mut bytes = payload_buffer(size, budget, limits, context.entry.path())?;
    let mut reader = RequiredReader::open(
        context.payload,
        context.entry.path(),
        size,
        budget,
        writer.checkpoint,
        limits,
    )?;
    reader
        .read_exact(&mut bytes)
        .map_err(|cause| error::io(Path::new(context.entry.path()), cause))?;
    Ok(bytes)
}

fn payload_buffer(
    size: u64,
    budget: &InputBudget,
    limits: Limits,
    name: &str,
) -> Result<Vec<u8>, MediaError> {
    if size > budget.remaining_bytes(limits) {
        return Err(MediaError::Limit {
            budget: "source_bytes",
            limit: limits.source_bytes,
        });
    }
    let native_size = usize::try_from(size)
        .map_err(|_| MediaError::Contract("retained input exceeds native address width".into()))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(native_size).map_err(|cause| {
        error::io(
            Path::new(name),
            io::Error::new(io::ErrorKind::OutOfMemory, cause),
        )
    })?;
    bytes.resize(native_size, 0);
    Ok(bytes)
}

pub(crate) struct RequiredReader<'input> {
    file: File,
    name: &'input str,
    size: u64,
    position: u64,
    budget: &'input InputBudget,
    checkpoint: Checkpoint<'input>,
    limits: Limits,
}

impl<'input> RequiredReader<'input> {
    pub(crate) fn open(
        path: &Path,
        name: &'input str,
        size: u64,
        budget: &'input InputBudget,
        checkpoint: Checkpoint<'input>,
        limits: Limits,
    ) -> Result<Self, MediaError> {
        budget.metadata(0, 1, limits, checkpoint)?;
        let result = File::open(path).map_err(|cause| error::io(Path::new(name), cause));
        let file = checked(result, checkpoint)?;
        let reader = Self {
            file,
            name,
            size,
            position: 0,
            budget,
            checkpoint,
            limits,
        };
        reader.check_length()?;
        Ok(reader)
    }

    pub(crate) fn read_payload(&mut self) -> Result<Vec<u8>, MediaError> {
        if self.position != 0 {
            return Err(MediaError::Contract(
                "whole-payload read must start at byte zero".into(),
            ));
        }
        let mut bytes = payload_buffer(self.size, self.budget, self.limits, self.name)?;
        self.read_exact(&mut bytes)
            .map_err(|cause| error::io(Path::new(self.name), cause))?;
        Ok(bytes)
    }

    pub(crate) fn read_chunk(&mut self, range: Range<u64>) -> Result<Vec<u8>, MediaError> {
        let size = range
            .end
            .checked_sub(range.start)
            .filter(|_| range.end <= self.size)
            .ok_or_else(|| MediaError::Contract("chunk exceeds its verified Entry range".into()))?;
        if size > self.limits.chunk_bytes {
            return Err(MediaError::Limit {
                budget: "chunk_bytes",
                limit: self.limits.chunk_bytes,
            });
        }
        let mut bytes = payload_buffer(size, self.budget, self.limits, self.name)?;
        self.seek(SeekFrom::Start(range.start))
            .and_then(|_| self.read_exact(&mut bytes))
            .map_err(|cause| error::io(Path::new(self.name), cause))?;
        Ok(bytes)
    }

    fn check_length(&self) -> Result<(), MediaError> {
        let result = self
            .file
            .metadata()
            .map_err(|cause| error::io(Path::new(self.name), cause));
        let metadata = checked(result, self.checkpoint)?;
        if metadata.len() != self.size {
            return Err(error::io(
                Path::new(self.name),
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "required payload length differs from its verified receipt",
                ),
            ));
        }
        Ok(())
    }

    fn read_checked(&mut self, output: &mut [u8]) -> Result<usize, MediaError> {
        self.budget.metadata(0, 1, self.limits, self.checkpoint)?;
        self.check_length()?;
        let remaining = self.size - self.position;
        let count = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(output.len())
            .min(IO_BLOCK_BYTES);
        if count == 0 {
            return Ok(0);
        }
        let available = self.budget.remaining_bytes(self.limits);
        let count_u64 = u64::try_from(count)
            .map_err(|_| MediaError::Contract("input read count exceeds u64".into()))?;
        if count_u64 > available {
            return Err(MediaError::Limit {
                budget: "source_bytes",
                limit: self.limits.source_bytes,
            });
        }
        let buffer = output
            .get_mut(..count)
            .ok_or_else(|| MediaError::Contract("invalid required input buffer".into()))?;
        let result = self
            .file
            .read(buffer)
            .map_err(|cause| error::io(Path::new(self.name), cause))
            .and_then(|read| {
                if read == 0 {
                    return Err(error::io(
                        Path::new(self.name),
                        io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "required input ended before its verified length",
                        ),
                    ));
                }
                let actual = u64::try_from(read)
                    .map_err(|_| MediaError::Contract("input read count exceeds u64".into()))?;
                self.budget.bytes(actual, self.limits)?;
                self.position += actual;
                Ok(read)
            });
        checked(result, self.checkpoint)
    }

    fn seek_checked(&mut self, from: SeekFrom) -> Result<u64, MediaError> {
        self.budget.metadata(0, 1, self.limits, self.checkpoint)?;
        self.check_length()?;
        let position = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.size) + i128::from(offset),
        };
        let position = u64::try_from(position).map_err(|_| {
            error::io(
                Path::new(self.name),
                io::Error::new(io::ErrorKind::InvalidInput, "negative input seek"),
            )
        })?;
        if position > self.size {
            return Err(error::io(
                Path::new(self.name),
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "input seek exceeds its verified range",
                ),
            ));
        }
        let result = self
            .file
            .seek(SeekFrom::Start(position))
            .map_err(|cause| error::io(Path::new(self.name), cause));
        self.position = checked(result, self.checkpoint)?;
        Ok(self.position)
    }
}

impl Read for RequiredReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.read_checked(output).map_err(io::Error::other)
    }
}

impl Seek for RequiredReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.seek_checked(position).map_err(io::Error::other)
    }
}

fn checked<T>(result: Result<T, MediaError>, checkpoint: Checkpoint<'_>) -> Result<T, MediaError> {
    match (result, checkpoint.check()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(operation), Err(checkpoint)) => Err(MediaError::InputCheckFailed {
            operation: Box::new(operation),
            checkpoint: Box::new(checkpoint),
        }),
    }
}

#[cfg(test)]
#[path = "required_reader_fault_tests.rs"]
mod fault_tests;

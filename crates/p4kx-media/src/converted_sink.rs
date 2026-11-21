// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::checkpoint::IO_BLOCK_BYTES;
use crate::error;
use crate::hashing;
use crate::output_operation;
use crate::temporary_file::TemporaryFile;
use crate::{Algorithm, ByteScope, Digest, MediaError, MediaWriter};

pub(crate) struct Encoded {
    pub(crate) digest: Digest,
    pub(crate) size: u64,
}

pub(crate) struct ConvertedSink<'writer, 'file, 'job> {
    writer: &'writer mut MediaWriter<'job>,
    temporary: &'file mut TemporaryFile<'job>,
    hash: Sha256,
    size: u64,
    failure: Option<Arc<MediaError>>,
    stop: LatchedStop,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LatchedStop {
    None,
    Cancelled,
    Changed,
}

impl<'writer, 'file, 'job> ConvertedSink<'writer, 'file, 'job> {
    pub(crate) fn new(
        writer: &'writer mut MediaWriter<'job>,
        temporary: &'file mut TemporaryFile<'job>,
    ) -> Self {
        Self {
            writer,
            temporary,
            hash: Sha256::new(),
            size: 0,
            failure: None,
            stop: LatchedStop::None,
        }
    }

    pub(crate) fn finish(self, callback: Result<(), MediaError>) -> Result<Encoded, MediaError> {
        let callback = callback.map_err(error::input);
        let checkpoint = self.completion_check(callback.as_ref().err());
        let result = self.complete(callback);
        output_operation::checked(result, checkpoint)
    }

    fn complete(self, callback: Result<(), MediaError>) -> Result<Encoded, MediaError> {
        if let Some(cause) = self.failure {
            return Err(match callback {
                Err(callback) if contains(&callback, cause.as_ref()) => error::input(callback),
                callback => MediaError::Output {
                    cause,
                    callback: callback.err().map(Box::new),
                },
            });
        }
        callback?;
        Ok(Encoded {
            size: self.size,
            digest: Digest {
                algorithm: Algorithm::Sha256,
                scope: ByteScope::ArtifactFile,
                hex: hashing::hex(self.hash.finalize().into()),
            },
        })
    }

    fn completion_check(&self, callback: Option<&MediaError>) -> Result<(), MediaError> {
        let checkpoint = self.writer.checkpoint();
        if let (Some(callback), Err(final_cause)) = (callback, &checkpoint)
            && self.writer.checkpoint.watch.is_changed()
            && repeated_change(callback, final_cause)
        {
            return Ok(());
        }
        match checkpoint {
            Err(MediaError::Cancelled)
                if self.stop == LatchedStop::Cancelled
                    && self.writer.checkpoint.cancel.is_cancelled() =>
            {
                Ok(())
            }
            Err(MediaError::Changed(_))
                if self.stop == LatchedStop::Changed
                    && self.writer.checkpoint.watch.is_changed() =>
            {
                Ok(())
            }
            result => result,
        }
    }

    fn observed_stop(&self, cause: &MediaError) -> LatchedStop {
        let cause = match cause {
            MediaError::OutputCheckFailed { checkpoint, .. } => checkpoint.as_ref(),
            cause => cause,
        };
        match cause {
            MediaError::Cancelled if self.writer.checkpoint.cancel.is_cancelled() => {
                LatchedStop::Cancelled
            }
            MediaError::Changed(_) if self.writer.checkpoint.watch.is_changed() => {
                LatchedStop::Changed
            }
            _ => LatchedStop::None,
        }
    }

    fn write_checked(&mut self, bytes: &[u8]) -> Result<usize, MediaError> {
        self.writer.checkpoint()?;
        let result = self.write_one(bytes);
        output_operation::checked(result, self.writer.checkpoint())
    }

    fn write_one(&mut self, bytes: &[u8]) -> Result<usize, MediaError> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.temporary.ensure_writable()?;
        let available = self.writer.remaining_output_bytes();
        if available == 0 {
            return Err(MediaError::Limit {
                budget: "output_bytes",
                limit: self.writer.limits.output_bytes,
            });
        }
        let count = bytes
            .len()
            .min(IO_BLOCK_BYTES)
            .min(usize::try_from(available).unwrap_or(usize::MAX));
        let bytes = bytes
            .get(..count)
            .ok_or_else(|| MediaError::Contract("invalid converted output range".into()))?;
        self.temporary
            .write(bytes)
            .map_err(|cause| error::io(Path::new("converted artifact"), cause))
            .and_then(|written| {
                if written == 0 {
                    return Err(error::io(
                        Path::new("converted artifact"),
                        io::Error::new(
                            io::ErrorKind::WriteZero,
                            "converted output accepted no bytes",
                        ),
                    ));
                }
                let actual = bytes.get(..written).ok_or_else(|| {
                    MediaError::Contract("invalid converted output byte count".into())
                })?;
                let written_u64 = u64::try_from(written).map_err(|_| {
                    MediaError::Contract("converted output byte count exceeds u64".into())
                })?;
                self.writer.reserve_output(written_u64)?;
                self.size = self
                    .size
                    .checked_add(written_u64)
                    .ok_or(MediaError::Limit {
                        budget: "output_bytes",
                        limit: self.writer.limits.output_bytes,
                    })?;
                self.hash.update(actual);
                Ok(written)
            })
    }

    fn flush_checked(&mut self) -> Result<(), MediaError> {
        self.writer.checkpoint()?;
        let result = (|| {
            self.temporary.ensure_writable()?;
            self.temporary
                .flush()
                .map_err(|cause| error::io(Path::new("converted artifact"), cause))
        })();
        output_operation::checked(result, self.writer.checkpoint())
    }

    fn fail(&mut self, cause: MediaError) -> io::Error {
        self.stop = self.observed_stop(&cause);
        let cause = Arc::new(cause);
        self.failure = Some(Arc::clone(&cause));
        io::Error::other(MediaError::Output {
            cause,
            callback: None,
        })
    }

    fn previous_failure(&self) -> Option<io::Error> {
        self.failure.as_ref().map(|cause| {
            io::Error::other(MediaError::Output {
                cause: Arc::clone(cause),
                callback: None,
            })
        })
    }
}

impl Write for ConvertedSink<'_, '_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(error) = self.previous_failure() {
            return Err(error);
        }
        self.write_checked(bytes).map_err(|cause| self.fail(cause))
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(error) = self.previous_failure() {
            return Err(error);
        }
        self.flush_checked().map_err(|cause| self.fail(cause))
    }
}

fn contains(cause: &MediaError, expected: &MediaError) -> bool {
    use std::error::Error;
    let mut current: Option<&(dyn Error + 'static)> = Some(cause);
    while let Some(error) = current {
        if error
            .downcast_ref::<MediaError>()
            .is_some_and(|cause| std::ptr::eq(cause, expected))
        {
            return true;
        }
        current = error.source();
    }
    false
}

fn repeated_change(callback: &MediaError, checkpoint: &MediaError) -> bool {
    use std::error::Error;
    let MediaError::Changed(final_change) = checkpoint else {
        return false;
    };
    let mut current: Option<&(dyn Error + 'static)> = Some(callback);
    while let Some(error) = current {
        if let Some(cause) = error.downcast_ref::<MediaError>() {
            let cause = match cause {
                MediaError::OutputCheckFailed { checkpoint, .. }
                | MediaError::InputCheckFailed { checkpoint, .. } => checkpoint.as_ref(),
                cause => cause,
            };
            if matches!(cause, MediaError::Changed(original) if original.path() == final_change.path())
            {
                return true;
            }
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
#[path = "converted_sink_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "converted_latch_tests.rs"]
mod latch_tests;

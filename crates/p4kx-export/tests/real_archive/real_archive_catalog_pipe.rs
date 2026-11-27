// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, PipeReader, PipeWriter, Read};
use std::thread;

use p4kx_export::CatalogError;

pub(super) fn transfer<T, Captured, ReadError>(
    consume: impl FnOnce(PipeReader) -> Result<Captured, ReadError> + Send,
    produce: impl FnOnce(PipeWriter) -> Result<(T, PipeWriter), CatalogError>,
) -> Result<(T, Captured), Box<dyn Error>>
where
    Captured: Send,
    ReadError: Error + Send + 'static,
{
    let (reader, writer) = io::pipe()?;
    thread::scope(|scope| {
        let consumer = scope.spawn(move || consume(reader));
        let producer = produce(writer).map(|(result, writer)| {
            drop(writer);
            result
        });
        let parsed = consumer
            .join()
            .map_err(|_| io::Error::other("catalog stream reader panicked"))?;
        match (producer, parsed) {
            (Ok(result), Ok(captured)) => Ok((result, captured)),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(CatalogError::Write(error)), Err(parse))
                if error.kind() == io::ErrorKind::BrokenPipe =>
            {
                Err(parse.into())
            }
            (Err(error), _) => Err(error.into()),
        }
    })
}

pub(super) struct CountingReader<R> {
    input: R,
    bytes: u64,
}

impl<R> CountingReader<R> {
    pub(super) const fn new(input: R) -> Self {
        Self { input, bytes: 0 }
    }

    pub(super) const fn byte_count(&self) -> u64 {
        self.bytes
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.input.read(buffer)?;
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(read).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("catalog stream byte count overflow"))?;
        Ok(read)
    }
}

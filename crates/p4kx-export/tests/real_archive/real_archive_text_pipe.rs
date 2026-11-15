// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufReader, PipeWriter, Read};
use std::thread;

use p4kx_export::CatalogError;
use serde_json::Value;

use super::{TestResult, TextPaths};

#[derive(Debug)]
pub(super) struct Capture {
    pub(super) document: Value,
    pub(super) bytes: u64,
}

pub(super) fn transfer<T>(
    paths: &TextPaths,
    produce: impl FnOnce(PipeWriter) -> Result<(T, PipeWriter), CatalogError>,
) -> TestResult<(T, Capture)> {
    let (reader, writer) = io::pipe()?;
    thread::scope(|scope| {
        let consumer = scope.spawn(move || {
            let mut input = BufReader::new(CountingReader {
                input: reader,
                bytes: 0,
            });
            let document = paths.read(&mut input)?;
            Ok::<_, serde_json::Error>(Capture {
                document,
                bytes: input.into_inner().bytes,
            })
        });
        let producer = produce(writer).map(|(result, writer)| {
            drop(writer);
            result
        });
        let parsed = consumer
            .join()
            .map_err(|_| io::Error::other("text census reader panicked"))?;
        match (producer, parsed) {
            (Ok(result), Ok(capture)) => Ok((result, capture)),
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

struct CountingReader<R> {
    input: R,
    bytes: u64,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.input.read(buffer)?;
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(read).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("text census byte count overflow"))?;
        Ok(read)
    }
}

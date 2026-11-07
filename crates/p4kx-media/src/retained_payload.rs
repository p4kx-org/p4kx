// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read};

use crate::{MediaError, MediaWriter, SourceContext};

const INPUT_CHUNK_BYTES: usize = 16 << 10;
pub(crate) fn read(
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Vec<u8>, MediaError> {
    let mut file = File::open(context.payload).map_err(|source| input_error(context, source))?;
    let actual_size = file
        .metadata()
        .map_err(|source| input_error(context, source))?
        .len();
    if actual_size != context.source.size {
        return Err(input_error(
            context,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "retained source payload size differs from source inventory",
            ),
        ));
    }
    let size = usize::try_from(actual_size).map_err(|_| {
        MediaError::Contract("bounded source does not fit native address space".into())
    })?;
    read_declared(&mut file, size, context, writer)
}

fn read_declared(
    file: &mut File,
    size: usize,
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<Vec<u8>, MediaError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|error| input_error(context, io::Error::new(io::ErrorKind::OutOfMemory, error)))?;
    let mut buffer = [0; INPUT_CHUNK_BYTES];
    while bytes.len() < size {
        writer.checkpoint()?;
        let remaining = size - bytes.len();
        let chunk = buffer
            .get_mut(..remaining.min(INPUT_CHUNK_BYTES))
            .ok_or_else(|| MediaError::Contract("source read chunk exceeds fixed buffer".into()))?;
        let count = file
            .read(chunk)
            .map_err(|source| input_error(context, source))?;
        if count == 0 {
            return Err(input_error(
                context,
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "retained source payload ended before its inventoried length",
                ),
            ));
        }
        let read = chunk.get(..count).ok_or_else(|| {
            MediaError::Contract("source read count exceeds supplied buffer".into())
        })?;
        bytes.extend_from_slice(read);
    }
    require_end(file, context, writer)?;
    Ok(bytes)
}

fn require_end(
    file: &mut File,
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    writer.checkpoint()?;
    let mut extra = [0; 1];
    if file
        .read(&mut extra)
        .map_err(|source| input_error(context, source))?
        != 0
    {
        return Err(input_error(
            context,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "retained source payload grew while being read",
            ),
        ));
    }
    Ok(())
}

fn input_error(context: &SourceContext<'_>, source: io::Error) -> MediaError {
    MediaError::Io {
        path: context.payload.to_path_buf(),
        source,
    }
}

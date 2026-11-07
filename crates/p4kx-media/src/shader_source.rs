// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read};

use crate::shader_payload::{
    ShaderBudget, ShaderDefinitionFailure, ShaderFailureReason, ShaderPayload,
};
use crate::shader_span::{ShaderOffsetBasis, ShaderSpan};
use crate::{
    MediaError, MediaIssue, MediaWriter, Projection, Severity, SourceContext, Status, Subresource,
};
use crate::{shader_diagnostic, shader_projection, shader_values};

const INPUT_CHUNK_BYTES: usize = 16 << 10;

pub(crate) fn project(
    context: &SourceContext<'_>,
    limits: p4kx_shader::Limits,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let bound = shader_values::count(limits.source_bytes)?;
    if context.source.size > bound {
        let failure = ShaderFailureReason::Limit {
            budget: ShaderBudget::SourceBytes,
            limit: bound,
        };
        let span = ShaderSpan {
            basis: ShaderOffsetBasis::Entry,
            start: 0,
            end_exclusive: context.source.size,
        };
        return failed(context, failure, span, writer);
    }
    let bytes = payload(context, writer)?;
    match p4kx_shader::read_with(&bytes, limits, || writer.checkpoint()) {
        Ok(definition) => shader_projection::project(context, &definition, writer),
        Err(p4kx_shader::ReadError::Syntax(error)) => failed(
            context,
            shader_diagnostic::failure(error)?,
            ShaderSpan::from_reader(error.span)?,
            writer,
        ),
        Err(p4kx_shader::ReadError::Interrupted(error)) => Err(error),
        Err(other) => Err(input_error(context, io::Error::other(other))),
    }
}

fn payload(context: &SourceContext<'_>, writer: &MediaWriter<'_>) -> Result<Vec<u8>, MediaError> {
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
                "retained shader payload size differs from source inventory",
            ),
        ));
    }
    let size = usize::try_from(actual_size).map_err(|_| {
        MediaError::Contract("bounded shader source does not fit native address space".into())
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
            .ok_or_else(|| MediaError::Contract("shader read chunk exceeds fixed buffer".into()))?;
        let count = file
            .read(chunk)
            .map_err(|source| input_error(context, source))?;
        if count == 0 {
            return Err(input_error(
                context,
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "retained shader payload ended before its inventoried length",
                ),
            ));
        }
        let read = chunk.get(..count).ok_or_else(|| {
            MediaError::Contract("shader read count exceeds supplied buffer".into())
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
                "retained shader payload grew while being read",
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

pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit_once(['/', '\\']).map_or(path, |(_, name)| name)
}

fn failed(
    context: &SourceContext<'_>,
    failure: ShaderFailureReason,
    span: ShaderSpan,
    writer: &mut MediaWriter<'_>,
) -> Result<Projection, MediaError> {
    let detail = format!(
        "Structured shader projection failed at original bytes {}..{}: {failure:?}",
        span.start, span.end_exclusive
    );
    let location = format!("entry.bytes[{}..{})", span.start, span.end_exclusive);
    let data = ShaderPayload::DefinitionFailure(ShaderDefinitionFailure {
        file_name: file_name(context.entry.path()).to_owned(),
        failure,
        span,
        counts: (),
    });
    writer.record(
        &Subresource::new(shader_projection::FAILURE_NAMESPACE, Vec::new()),
        Status::Failed,
        &data,
        &[MediaIssue::new(
            "shader.definition.failed",
            location,
            Severity::Error,
            detail,
        )],
    )?;
    Ok(Projection::Written)
}

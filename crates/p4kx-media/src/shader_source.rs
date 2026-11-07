// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::shader_payload::{
    ShaderBudget, ShaderDefinitionFailure, ShaderFailureReason, ShaderPayload,
};
use crate::shader_span::{ShaderOffsetBasis, ShaderSpan};
use crate::{
    MediaError, MediaIssue, MediaWriter, Projection, Severity, SourceContext, Status, Subresource,
};
use crate::{shader_diagnostic, shader_projection, shader_values};

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
    let bytes = crate::retained_payload::read(context, writer)?;
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

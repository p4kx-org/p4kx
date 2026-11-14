// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use crate::hashing;
use crate::output_operation;
use crate::output_path;
use crate::publication::Publication;
use crate::temporary_file::TemporaryFile;
use crate::{Artifact, ByteScope, MediaError, MediaWriter};

pub(crate) fn temporary<'job>(
    writer: &MediaWriter<'job>,
    directory: &Path,
    missing: &'static str,
) -> Result<TemporaryFile<'job>, MediaError> {
    writer.checkpoint()?;
    let result = (|| {
        writer
            .namespace
            .ensure_namespace_writable(directory, writer.checkpoint.cancel.flag())?;
        if !output_path::directory(directory)? {
            return Err(MediaError::Destination(missing.into()));
        }
        TemporaryFile::create(directory, writer.checkpoint)
    })();
    match result {
        Ok(temporary) => match writer.checkpoint() {
            Ok(()) => Ok(temporary),
            Err(cause) => temporary.finish(Err(cause)),
        },
        Err(cause) => output_operation::checked(Err(cause), writer.checkpoint()),
    }
}

pub(crate) fn place(
    writer: &MediaWriter<'_>,
    artifact: &Artifact,
    temporary: &mut TemporaryFile<'_>,
    collision: &'static str,
) -> Result<bool, MediaError> {
    writer.checkpoint()?;
    let result = place_checked(writer, artifact, temporary, collision);
    output_operation::checked(result, writer.checkpoint())
}

fn place_checked(
    writer: &MediaWriter<'_>,
    artifact: &Artifact,
    temporary: &mut TemporaryFile<'_>,
    collision: &'static str,
) -> Result<bool, MediaError> {
    let path = writer.root.join(&artifact.path);
    writer
        .namespace
        .ensure_namespace_writable(&path, writer.checkpoint.cancel.flag())?;
    if output_path::file(&path)? {
        let (digest, actual) = hashing::file(
            &path,
            ByteScope::ArtifactFile,
            artifact.size,
            &writer.checkpoint,
        )?;
        if digest != artifact.digest || actual != artifact.size {
            return Err(MediaError::Contract(collision.into()));
        }
        Ok(false)
    } else {
        temporary.persist(&path, Publication::Namespace(writer.namespace))?;
        Ok(true)
    }
}

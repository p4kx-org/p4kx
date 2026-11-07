// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::artifact::BoundedLine;
use crate::checkpoint::Checkpoint;
use crate::error;
use crate::generation::Boundary;
use crate::observed_write::ObservedWrite;
use crate::output_path;
use crate::{Index, MediaError};

pub(crate) fn write(
    index: &Index,
    path: &Path,
    checkpoint: &Checkpoint<'_>,
    observe: &impl Fn(Boundary) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let parent = path
        .parent()
        .ok_or_else(|| MediaError::Destination("index has no parent".into()))?;
    p4kx_extraction::ensure_writable(path)?;
    output_path::ensure_directory(parent)?;
    output_path::file(path)?;
    let mut line = BoundedLine::new(index.limits.record_bytes);
    let serialised = serde_json::to_writer_pretty(ObservedWrite::new(&mut line, checkpoint), index);
    checkpoint.check()?;
    serialised?;
    line.write_all(b"\n").map_err(|_| MediaError::Limit {
        budget: "index_bytes",
        limit: index.limits.record_bytes,
    })?;
    checkpoint.finish()?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| error::io(parent, source))?;
    temporary
        .write_all(&line.bytes)
        .map_err(|source| error::io(path, source))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| error::io(path, source))?;
    observe(Boundary::IndexSynced)?;
    checkpoint.finish()?;
    observe(Boundary::BeforeIndexReplace)?;
    output_path::ensure_directory(parent)?;
    output_path::file(path)?;
    temporary
        .persist(path)
        .map_err(|failure| error::io(path, failure.error))?;
    #[cfg(unix)]
    crate::generation::sync_directory(parent)?;
    Ok(())
}

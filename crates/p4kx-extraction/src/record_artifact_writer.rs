// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_dataforge::{DataForge, PositionedRecord, Record, TreeBuilder};

use super::{RecordOutcome, RecordRun};
use crate::entry_path::relative_path;
use crate::file_receipt::{
    OrdinaryReplacement, StagedReceiptPublication, companion_path, finish_staged_receipt,
};
use crate::folder_cache::FolderCache;
use crate::origin_file_io::{self, StageDisposition};
use crate::output_creation::create_temporary_protected;
use crate::output_file::StagedOutput;
use crate::portable_name::{check_path_length, fold};
use crate::write::write_error;
use crate::{
    ConflictPolicy, DataForgeEntry, ExtractionError, FileReceipt, OriginDigestWriter,
    RecordExportError, RecordFileFormat, RecordJsonReceipt,
};

type Render<F> = fn(
    &mut F,
    &DataForgeEntry,
    &DataForge<'_>,
    &PositionedRecord<'_>,
) -> io::Result<RecordJsonReceipt>;

pub(super) struct Observation<'job, F> {
    input: &'job DataForgeEntry,
    render: Render<F>,
}

impl<'job, F> Observation<'job, F> {
    pub(super) const fn new(input: &'job DataForgeEntry, render: Render<F>) -> Self {
        Self { input, render }
    }
}

#[derive(Default)]
pub(super) struct Scratch<'source, F> {
    trees: TreeBuilder<'source>,
    format: F,
    bytes: Vec<u8>,
    read: Vec<u8>,
}

pub(super) struct Writer<'job, 'source, F> {
    run: &'job RecordRun<'job>,
    target: &'job Path,
    forge: &'job DataForge<'source>,
    observation: Option<Observation<'job, F>>,
    reserved: HashSet<String>,
    folders: FolderCache,
}

impl<'job, 'source, F: RecordFileFormat> Writer<'job, 'source, F> {
    pub(super) fn new(
        run: &'job RecordRun<'job>,
        target: &'job Path,
        forge: &'job DataForge<'source>,
        observation: Option<Observation<'job, F>>,
        reserved: HashSet<String>,
    ) -> Self {
        Self {
            run,
            target,
            forge,
            observation,
            reserved,
            folders: FolderCache::default(),
        }
    }

    pub(super) fn write<'record>(
        &self,
        record: &Record<'record>,
        file: &str,
        scratch: &mut Scratch<'record, F>,
    ) -> RecordOutcome {
        if (self.run.stop)() {
            return RecordOutcome::Cancelled;
        }
        match self.try_write(record, file, scratch) {
            Ok(outcome) => outcome,
            Err(RecordExportError::Write(error))
                if crate::guard_cancellation::extraction_cancelled(&error) =>
            {
                RecordOutcome::Cancelled
            }
            Err(error) => RecordOutcome::Failed(error),
        }
    }

    fn try_write<'record>(
        &self,
        record: &Record<'record>,
        file: &str,
        scratch: &mut Scratch<'record, F>,
    ) -> Result<RecordOutcome, RecordExportError> {
        let relative = relative_path(file)?;
        check_path_length(&self.target.join(&relative)).map_err(ExtractionError::from)?;
        let name = relative.file_name().ok_or(ExtractionError::UnsafePath)?;
        let folder = self.folders.folder_protected(
            self.target,
            relative.parent(),
            self.run.protection,
            self.run.cancel,
        )?;
        let destination = folder.join(name);
        let rendered = self.render(record, &destination, scratch)?;
        let bytes = rendered
            .receipt
            .as_ref()
            .map_or(scratch.bytes.as_slice(), RecordJsonReceipt::bytes);
        if crate::record_existing::keep(&destination, self.run.policy, bytes, &mut scratch.read)? {
            return Ok(rendered.kept(file));
        }

        let (temporary, part, owner) =
            create_temporary_protected(&folder, self.run.protection, self.run.cancel)?;
        let staged = StagedOutput::new(
            &temporary,
            &destination,
            &owner,
            self.run.protection,
            self.run.cancel,
        );
        let placed = self.stage(&staged, part, bytes, &rendered, file);
        match placed {
            Ok(StageDisposition::Moved(outcome)) => Ok(outcome),
            Ok(StageDisposition::Cleanup(outcome)) => {
                crate::output_cleanup::extraction(Ok(outcome), &owner, &temporary, self.run.cancel)
                    .map_err(Into::into)
            }
            Err(error) => {
                crate::output_cleanup::extraction(Err(error), &owner, &temporary, self.run.cancel)
                    .map_err(Into::into)
            }
        }
    }

    fn render<'record>(
        &self,
        record: &Record<'record>,
        destination: &Path,
        scratch: &mut Scratch<'record, F>,
    ) -> Result<Rendered, RecordExportError> {
        scratch.bytes.clear();
        let write_error = |source| write_error(destination.to_path_buf(), source);
        if let Some(observation) = &self.observation {
            let tree = scratch.trees.build_positioned_partial(record)?;
            let receipt =
                (observation.render)(&mut scratch.format, observation.input, self.forge, &tree)
                    .map_err(write_error)?;
            return Ok(Rendered {
                errors: tree.tree().error_count(),
                receipt: Some(receipt),
            });
        }
        let tree = scratch.trees.build(record)?;
        scratch
            .format
            .write_file(&tree, self.run.annotations, &mut scratch.bytes)
            .map_err(write_error)?;
        Ok(Rendered {
            errors: tree.error_count(),
            receipt: None,
        })
    }

    fn stage(
        &self,
        staged: &StagedOutput<'_>,
        mut part: fs::File,
        bytes: &[u8],
        rendered: &Rendered,
        file: &str,
    ) -> Result<StageDisposition<RecordOutcome>, ExtractionError> {
        let output = staged.output();
        let companion = rendered
            .receipt
            .as_ref()
            .map(|artifact| self.companion(output, file).map(|name| (artifact, name)));
        if let Some(Ok((artifact, companion))) = &companion {
            let mut measured = OriginDigestWriter::new(part);
            measured
                .write_all(bytes)
                .map_err(|source| write_error(output.to_path_buf(), source))?;
            let digest = measured
                .finish()
                .map_err(|source| write_error(output.to_path_buf(), source))?;
            return self.publish(staged, digest, artifact, companion, rendered, file);
        }
        part.write_all(bytes)
            .map_err(|source| write_error(output.to_path_buf(), source))?;
        drop(part);
        let origin_failure = companion.and_then(Result::err);
        self.replace(staged, rendered, file, origin_failure)
    }

    fn companion(&self, output: &Path, file: &str) -> Result<PathBuf, ExtractionError> {
        let path =
            companion_path(output).map_err(|source| write_error(output.to_path_buf(), source))?;
        let relative = format!("{file}{}", crate::file_receipt::COMPANION_SUFFIX);
        if self.reserved.contains(&fold(&relative).collect::<String>()) {
            return Err(write_error(
                path,
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "a selected Record directory needs the companion path",
                ),
            ));
        }
        check_path_length(&path)?;
        Ok(path)
    }

    fn publish(
        &self,
        staged: &StagedOutput<'_>,
        digest: crate::OriginArtifactDigest,
        artifact: &RecordJsonReceipt,
        companion: &Path,
        rendered: &Rendered,
        file: &str,
    ) -> Result<StageDisposition<RecordOutcome>, ExtractionError> {
        let output = staged.output();
        let origin = FileReceipt::for_native_staged_replacement(
            output,
            self.run.protection,
            self.run.cancel.cloned().unwrap_or_default(),
        )
        .map_err(|source| write_error(output.to_path_buf(), source))?;
        let publication = origin.publish_staged_with_policy(
            staged,
            digest,
            self.run.policy,
            || Ok(()),
            || {},
            |out, binding| artifact.describe(out, binding),
        );
        let (receipt, retirement) = match publication {
            None => return Ok(StageDisposition::Cleanup(rendered.kept(file))),
            Some(StagedReceiptPublication::Uncommitted(error)) => {
                return Err(write_error(output.to_path_buf(), error));
            }
            Some(StagedReceiptPublication::Committed {
                receipt,
                retirement,
            }) => (receipt, retirement),
        };
        let written = receipt.as_ref().is_some_and(Result::is_ok);
        let failure = if receipt.is_none() {
            Some(write_error(
                companion.to_path_buf(),
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "the companion path contains foreign data",
                ),
            ))
        } else {
            None
        };
        let failure = finish_staged_receipt(receipt, retirement)
            .err()
            .map(|error| write_error(companion.to_path_buf(), error))
            .or(failure);
        let result = rendered.written(written.then(|| companion.to_path_buf()), failure);
        Ok(if self.run.policy == ConflictPolicy::Skip {
            StageDisposition::Cleanup(result)
        } else {
            StageDisposition::Moved(result)
        })
    }

    fn replace(
        &self,
        staged: &StagedOutput<'_>,
        rendered: &Rendered,
        file: &str,
        origin_failure: Option<ExtractionError>,
    ) -> Result<StageDisposition<RecordOutcome>, ExtractionError> {
        let output = staged.output();
        if !F::EXTENSION.eq_ignore_ascii_case(".json") {
            staged.check()?;
            fs::rename(staged.temporary(), output)
                .map_err(|source| write_error(output.to_path_buf(), source))?;
            return Ok(StageDisposition::Moved(
                rendered.written(None, origin_failure),
            ));
        }
        let held = OrdinaryReplacement::new(output, self.run.protection, self.run.cancel)
            .map_err(|source| write_error(output.to_path_buf(), source))?;
        let replaced = replace_json(&held, staged, self.run.policy);
        let committed = matches!(replaced, Ok(true));
        let placed = held.finish_io(replaced);
        match placed {
            Ok(false) => Ok(StageDisposition::Cleanup(rendered.kept(file))),
            Ok(true) => Ok(self.placed(rendered.written(None, origin_failure))),
            Err(error) if committed => {
                Ok(self
                    .placed(rendered.written(None, Some(write_error(output.to_path_buf(), error)))))
            }
            Err(error) => Err(write_error(output.to_path_buf(), error)),
        }
    }

    fn placed(&self, outcome: RecordOutcome) -> StageDisposition<RecordOutcome> {
        if self.run.policy == ConflictPolicy::Skip {
            StageDisposition::Cleanup(outcome)
        } else {
            StageDisposition::Moved(outcome)
        }
    }
}

fn replace_json(
    held: &OrdinaryReplacement<'_>,
    staged: &StagedOutput<'_>,
    policy: ConflictPolicy,
) -> io::Result<bool> {
    staged.check().map_err(io::Error::other)?;
    if policy == ConflictPolicy::Skip {
        match fs::symlink_metadata(staged.output()) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    held.invalidate()?;
    if policy == ConflictPolicy::Skip {
        origin_file_io::install_without_replacing(staged, staged.output())
    } else {
        origin_file_io::replace_staged(staged).map(|()| true)
    }
}

struct Rendered {
    errors: usize,
    receipt: Option<RecordJsonReceipt>,
}

impl Rendered {
    fn kept(&self, file: &str) -> RecordOutcome {
        RecordOutcome::Kept {
            file: file.to_owned(),
            errors: self.errors,
        }
    }

    const fn written(
        &self,
        receipt: Option<PathBuf>,
        origin_failure: Option<ExtractionError>,
    ) -> RecordOutcome {
        RecordOutcome::Written {
            errors: self.errors,
            receipt,
            origin_failure,
        }
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_extraction::{
    Annotations, ExtractionError, Record, RecordExportError, RecordExportOptions, RecordStreamRows,
    TreeBuilder,
};

use super::{StreamEvidence, failure};
use crate::line_writer::LineWriter;
use crate::record_summary::RecordExportReceipt;
use crate::{NDJSON_FILE, RecordExportSummary, RecordForm};

/// One selected Record, in the order of the file.
pub(super) struct Line<'r, 'f> {
    pub(super) record: &'r Record<'f>,
}

/// The lines of one part of a batch and what happened to its Records.
pub(super) struct Part<'r, 'f, 'input, 'locale> {
    pub(super) bytes: Vec<u8>,
    error_counts: Vec<usize>,
    pub(super) failures: Vec<(&'r Record<'f>, RecordExportError)>,
    receipt: Option<RecordExportReceipt>,
    pub(super) origins: Option<Vec<RecordStreamRows<'input, 'locale>>>,
}

impl Part<'_, '_, '_, '_> {
    pub(super) fn account(self, summary: &mut RecordExportSummary) {
        if let (Some(receipt), Some(part)) = (&mut summary.receipt, self.receipt) {
            receipt.indices.extend(part.indices);
            receipt.rowless_indices.extend(part.rowless_indices);
        }
        summary.written = summary.written.saturating_add(self.error_counts.len());
        for errors in self.error_counts {
            summary.error_values = summary.error_values.saturating_add(errors);
            summary.records_with_errors = summary
                .records_with_errors
                .saturating_add(usize::from(errors > 0));
        }
        summary.failures.extend(
            self.failures
                .into_iter()
                .map(|(record, error)| failure(record, error)),
        );
    }
}

pub(super) struct Scratch<'f> {
    trees: TreeBuilder<'f>,
    lines: LineWriter,
}

impl<'f> Scratch<'f> {
    pub(super) fn new(form: RecordForm) -> Self {
        Self {
            trees: TreeBuilder::new(),
            lines: LineWriter::new(form),
        }
    }

    fn render<'input, 'locale>(
        &mut self,
        record: &Record<'f>,
        form: RecordForm,
        annotations: Annotations<'_>,
        part: &mut Part<'_, 'f, 'input, 'locale>,
        original: Option<StreamEvidence<'input, '_, 'locale>>,
    ) -> Result<usize, RecordExportError> {
        let write_error = |source| ExtractionError::Write {
            path: PathBuf::from(form.file().unwrap_or(NDJSON_FILE)),
            source,
        };
        if let (Some(origins), Some(original)) = (&mut part.origins, original) {
            let tree = self.trees.build_positioned_partial(record)?;
            let row = self
                .lines
                .write_observed(original, &tree, &mut part.bytes)
                .map_err(write_error)?;
            origins.push(row);
            return Ok(tree.tree().error_count());
        }
        let tree = self.trees.build(record)?;
        self.lines
            .write(&tree, annotations, &mut part.bytes)
            .map_err(write_error)?;
        Ok(tree.error_count())
    }

    pub(super) fn write_part<'r, 'input, 'locale>(
        &mut self,
        lines: &[Line<'r, 'f>],
        options: &RecordExportOptions<'_>,
        form: RecordForm,
        retain_receipt: bool,
        original: Option<StreamEvidence<'input, '_, 'locale>>,
    ) -> Part<'r, 'f, 'input, 'locale> {
        let mut part = Part {
            bytes: Vec::new(),
            error_counts: Vec::with_capacity(lines.len()),
            failures: Vec::new(),
            receipt: retain_receipt.then(RecordExportReceipt::default),
            origins: original.map(|_| Vec::with_capacity(lines.len())),
        };
        for line in lines {
            let start = part.bytes.len();
            match self.render(
                line.record,
                form,
                options.annotations(),
                &mut part,
                original,
            ) {
                Ok(errors) => {
                    part.error_counts.push(errors);
                    if let Some(receipt) = &mut part.receipt {
                        let index = line.record.index();
                        receipt.indices.push(index);
                        if part.bytes.len() == start {
                            receipt.rowless_indices.push(index);
                        }
                    }
                }
                Err(error) => {
                    part.bytes.truncate(start);
                    part.failures.push((line.record, error));
                }
            }
            if let Some(progress) = options.progress {
                progress();
            }
        }
        part
    }
}

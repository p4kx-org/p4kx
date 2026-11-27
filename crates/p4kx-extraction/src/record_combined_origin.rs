// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::output_file::StagedOutput;
use crate::record_format::{
    RecordOriginal, record_stream_publication, xml_receipt::WrappedRecordXml,
};
use crate::{
    ConflictPolicy, DataForgeOrigin, ExtractionError, OriginArtifactDigest, OriginCompanionSpool,
    RecordExportOptions, RecordExportReport, RecordStreamReceipt,
};
use std::io;

pub(super) struct Origin {
    original: RecordOriginal,
    spool: OriginCompanionSpool,
}

impl Origin {
    pub(super) fn new(input: &DataForgeOrigin) -> io::Result<Self> {
        let spool = OriginCompanionSpool::new()?;
        let original = RecordOriginal::observe(spool.sources(), input)?;
        Ok(Self { original, spool })
    }

    pub(super) fn append(&mut self, record: WrappedRecordXml<'_>, start: u64) -> io::Result<()> {
        record.stage(start, &self.original, &mut self.spool)
    }

    pub(super) fn finish_batch(&mut self) -> io::Result<()> {
        self.spool.finish_batch()
    }

    pub(super) fn finish(self, digest: OriginArtifactDigest) -> io::Result<RecordStreamReceipt> {
        Ok(RecordStreamReceipt::new(digest, self.spool.finish()?))
    }
}

pub(super) fn kept(name: String, report: &mut RecordExportReport) {
    report.skipped.push(name);
    report.written = 0;
    if let Some(indices) = &mut report.written_record_indices {
        indices.clear();
    }
}

pub(super) fn place(
    staged: &StagedOutput<'_>,
    origin: Option<RecordStreamReceipt>,
    options: &RecordExportOptions<'_>,
    report: &mut RecordExportReport,
    name: String,
) -> Result<bool, ExtractionError> {
    let Some(commit) = record_stream_publication::publish_staged(staged, options, origin)? else {
        kept(name, report);
        return Ok(false);
    };
    report.origin_receipts.extend(commit.companion);
    report.origin_failures.extend(commit.origin_failure);
    Ok(options.policy != ConflictPolicy::Skip)
}

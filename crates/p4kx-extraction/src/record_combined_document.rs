// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_dataforge::{
    Annotations, DataForge, RECORD_XML_NAMESPACE, Record, RecordXml, TreeBuilder,
};
use rayon::prelude::*;

use super::failure;
use super::origin::Origin;
use crate::counting::Counting;
use crate::record_format::xml_receipt::{WrappedRecordXml, XmlAnnotationSources, render_wrapped};
use crate::write::write_error;
use crate::{
    CancelToken, DataForgeOrigin, ExtractionError, OriginArtifactDigest, OriginDigestWriter,
    RecordExportError, RecordExportOptions, RecordExportReport, RecordFailure, RecordStreamReceipt,
};

const BATCH_LEN: usize = 1024;
const WRITE_BUFFER_LEN: usize = 256 << 10;
const HEAD: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<p4kx:records xmlns:p4kx=\"";

enum Payload<'entry> {
    Plain(Vec<u8>),
    Observed(WrappedRecordXml<'entry>),
}

impl Payload<'_> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Plain(bytes) => bytes,
            Self::Observed(xml) => &xml.bytes,
        }
    }
}

enum Built<'entry> {
    Xml {
        payload: Payload<'entry>,
        record_index: u32,
        error_values: usize,
    },
    Failed(RecordFailure),
    Cancelled,
}

#[derive(Default)]
struct Scratch<'f> {
    trees: TreeBuilder<'f>,
    xml: RecordXml,
}

impl<'source> Scratch<'source> {
    fn render_plain<'entry>(
        &mut self,
        record: &Record<'source>,
        annotations: Annotations<'_>,
        file: &Path,
    ) -> Result<(Payload<'entry>, usize), RecordExportError> {
        let tree = self.trees.build(record)?;
        let mut bytes = Vec::new();
        self.xml
            .write_wrapped_element_annotated(&tree, annotations, 1, &mut bytes)
            .map_err(|source| write_error(file.to_path_buf(), source))?;
        Ok((Payload::Plain(bytes), tree.error_count()))
    }

    fn render_observed<'entry>(
        &mut self,
        forge: &DataForge<'_>,
        record: &Record<'source>,
        file: &Path,
        sources: XmlAnnotationSources<'_, 'entry>,
    ) -> Result<(Payload<'entry>, usize), RecordExportError> {
        let record = self.trees.build_positioned_partial(record)?;
        let xml = render_wrapped(&mut self.xml, forge, &record, sources)
            .map_err(|source| write_error(file.to_path_buf(), source))?;
        Ok((Payload::Observed(xml), record.tree().error_count()))
    }
}

fn build<'source, 'entry>(
    record: &Record<'source>,
    scratch: &mut Scratch<'source>,
    cancel: Option<&CancelToken>,
    annotations: Annotations<'_>,
    file: &Path,
    observed: Option<&DataForge<'_>>,
    sources: XmlAnnotationSources<'_, 'entry>,
) -> Built<'entry> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Built::Cancelled;
    }
    let rendered = match observed {
        Some(forge) => scratch.render_observed(forge, record, file, sources),
        None => scratch.render_plain(record, annotations, file),
    };
    match rendered {
        Ok((payload, error_values)) => Built::Xml {
            payload,
            record_index: record.index(),
            error_values,
        },
        Err(error) => Built::Failed(failure(record, error)),
    }
}

pub(super) struct StagedDocument {
    pub(super) len: u64,
    pub(super) crc: u32,
    pub(super) origin: Option<RecordStreamReceipt>,
}

enum Body {
    Plain(BufWriter<File>),
    Observed(OriginDigestWriter<BufWriter<File>>),
}

impl Body {
    fn new(part: File, observed: bool) -> Self {
        let writer = BufWriter::with_capacity(WRITE_BUFFER_LEN, part);
        if observed {
            Self::Observed(OriginDigestWriter::new(writer))
        } else {
            Self::Plain(writer)
        }
    }

    fn finish(self) -> io::Result<Option<OriginArtifactDigest>> {
        match self {
            Self::Plain(mut writer) => writer.flush().map(|()| None),
            Self::Observed(writer) => writer.finish().map(Some),
        }
    }
}

impl Write for Body {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(writer) => writer.write(bytes),
            Self::Observed(writer) => writer.write(bytes),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(writer) => writer.flush(),
            Self::Observed(writer) => writer.flush(),
        }
    }
}

pub(super) struct Document<'p> {
    out: Counting<Body>,
    file: &'p Path,
    has_records: bool,
    origin: Option<Origin>,
}

impl<'p> Document<'p> {
    pub(super) fn new(
        part: File,
        file: &'p Path,
        original: Option<&DataForgeOrigin>,
    ) -> Result<Self, ExtractionError> {
        let origin = original
            .map(Origin::new)
            .transpose()
            .map_err(|source| write_error(file.to_path_buf(), source))?;
        let mut document = Self {
            out: Counting::new(Body::new(part, origin.is_some())),
            file,
            has_records: false,
            origin,
        };
        document.put(HEAD)?;
        document.put(RECORD_XML_NAMESPACE.as_bytes())?;
        document.put(b"\"")?;
        Ok(document)
    }

    /// Builds the next batch while the preceding one is written in document order.
    pub(super) fn stream(
        mut self,
        forge: &DataForge<'_>,
        records: &[Record<'_>],
        order: &[usize],
        options: &RecordExportOptions<'_>,
        sources: XmlAnnotationSources<'_, '_>,
        report: &mut RecordExportReport,
    ) -> Result<Option<StagedDocument>, ExtractionError> {
        let file = self.file;
        let observed = self.origin.as_ref().map(|_| forge);
        let (cancel, progress, annotations) =
            (options.cancel, options.progress, options.annotations());
        let mut pending = Vec::new();
        for batch in order.chunks(BATCH_LEN) {
            let (written, next) = rayon::join(
                || self.put_batch(std::mem::take(&mut pending), report),
                || {
                    batch
                        .par_iter()
                        .filter_map(|position| records.get(*position))
                        .map_init(Scratch::default, |scratch, record| {
                            let built = build(
                                record,
                                scratch,
                                cancel,
                                annotations,
                                file,
                                observed,
                                sources,
                            );
                            if let Some(progress) = progress {
                                progress();
                            }
                            built
                        })
                        .collect::<Vec<_>>()
                },
            );
            if !written? {
                return Ok(None);
            }
            pending = next;
        }
        if !self.put_batch(pending, report)? {
            return Ok(None);
        }
        self.finish().map(Some)
    }

    fn finish(mut self) -> Result<StagedDocument, ExtractionError> {
        self.put(if self.has_records {
            b"</p4kx:records>\n".as_slice()
        } else {
            b"/>\n".as_slice()
        })?;
        let len = self.out.len();
        let crc = self.out.crc();
        let write = |source| write_error(self.file.to_path_buf(), source);
        let digest = self.out.into_inner().finish().map_err(write)?;
        let origin = self
            .origin
            .zip(digest)
            .map(|(origin, digest)| origin.finish(digest))
            .transpose()
            .map_err(write)?;
        Ok(StagedDocument { len, crc, origin })
    }

    /// Writes one batch and reports whether all its workers reached a terminal Record outcome.
    fn put_batch(
        &mut self,
        batch: Vec<Built<'_>>,
        report: &mut RecordExportReport,
    ) -> Result<bool, ExtractionError> {
        for built in batch {
            match built {
                Built::Xml {
                    payload,
                    record_index,
                    error_values,
                } => {
                    self.put_record(payload)?;
                    report.written = report.written.saturating_add(1);
                    if let Some(indices) = &mut report.written_record_indices {
                        indices.push(record_index);
                    }
                    report.error_values = report.error_values.saturating_add(error_values);
                    report.records_with_errors = report
                        .records_with_errors
                        .saturating_add(usize::from(error_values > 0));
                }
                Built::Failed(failure) => report.failures.push(failure),
                Built::Cancelled => return Ok(false),
            }
        }
        if let Some(origin) = &mut self.origin {
            origin
                .finish_batch()
                .map_err(|source| write_error(self.file.to_path_buf(), source))?;
        }
        Ok(true)
    }

    fn put_record(&mut self, payload: Payload<'_>) -> Result<(), ExtractionError> {
        if !self.has_records {
            self.has_records = true;
            self.put(b">\n")?;
        }
        let start = self.out.len();
        self.put(payload.bytes())?;
        if let (Some(origin), Payload::Observed(record)) = (&mut self.origin, payload) {
            origin
                .append(record, start)
                .map_err(|source| write_error(self.file.to_path_buf(), source))?;
        }
        Ok(())
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), ExtractionError> {
        self.out
            .write_all(bytes)
            .map_err(|source| write_error(self.file.to_path_buf(), source))
    }
}

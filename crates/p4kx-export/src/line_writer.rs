// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{
    Annotations, FlatFormat, FlatValues, JsonLayout, PositionedRecord, RecordJson,
    RecordStreamRows, RecordTree,
};

use crate::RecordForm;
use crate::ndjson::StreamEvidence;

/// Writes the lines of one single-file Record export: a compact Record JSON object per Record, or
/// the rows of a flat value export.
pub(crate) enum LineWriter {
    Json(RecordJson),
    Flat(FlatValues),
}

impl LineWriter {
    /// Returns the writer of `form`, which must be one of the single-file forms.
    pub(crate) fn new(form: RecordForm) -> Self {
        match form {
            RecordForm::FlatCsv => Self::Flat(FlatValues::new(FlatFormat::Csv)),
            RecordForm::FlatCsvRaw => Self::Flat(FlatValues::new(FlatFormat::CsvRaw)),
            RecordForm::FlatCsvStrict => Self::Flat(FlatValues::new(FlatFormat::CsvStrict)),
            RecordForm::FlatNdjson => Self::Flat(FlatValues::new(FlatFormat::Ndjson)),
            RecordForm::Xml | RecordForm::Json | RecordForm::Ndjson => {
                Self::Json(RecordJson::new(JsonLayout::Compact))
            }
        }
    }

    /// Writes what starts the file.
    pub(crate) fn header(&self, with_text: bool, out: &mut Vec<u8>) -> io::Result<()> {
        match self {
            Self::Json(_) => Ok(()),
            Self::Flat(flat) => flat.write_header(with_text, out),
        }
    }

    pub(crate) fn write_observed<'input, 'locale>(
        &mut self,
        evidence: StreamEvidence<'input, '_, 'locale>,
        record: &PositionedRecord<'_>,
        out: &mut Vec<u8>,
    ) -> io::Result<RecordStreamRows<'input, 'locale>> {
        let StreamEvidence {
            input,
            locale,
            tags,
        } = evidence;
        match self {
            Self::Json(writer) => {
                RecordStreamRows::write_json(input, writer, record, out, locale, tags)
            }
            Self::Flat(writer) => RecordStreamRows::write_flat(input, writer, record, out, locale),
        }
    }

    /// Appends the line or lines of `tree` to `out`.
    pub(crate) fn write(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut Vec<u8>,
    ) -> io::Result<()> {
        match self {
            Self::Json(json) => {
                json.write_annotated(tree, annotations, out)?;
                out.push(b'\n');
                Ok(())
            }
            Self::Flat(flat) => flat.write_annotated(tree, annotations, out),
        }
    }
}

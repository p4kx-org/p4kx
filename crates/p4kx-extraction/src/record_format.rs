// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{Annotations, RecordJson, RecordTree, RecordXml};

#[path = "record_field_receipt.rs"]
mod field_receipt;
#[path = "record_flat_fields.rs"]
mod flat_fields;
#[path = "record_json_line.rs"]
pub(crate) mod json_line;
#[path = "record_json_receipt.rs"]
mod json_receipt;
#[path = "record_stream_input.rs"]
pub(crate) mod record_stream_input;
#[path = "record_stream_publication.rs"]
pub(crate) mod record_stream_publication;
#[path = "record_stream_fields.rs"]
pub(crate) mod stream_fields;
#[path = "record_xml_receipt.rs"]
pub(crate) mod xml_receipt;

pub(crate) use field_receipt::{RecordFieldReceipt, RecordOriginal};
pub use json_receipt::RecordJsonReceipt;

/// The file format of a Record export with one file per Record: how a Record tree becomes the
/// bytes of its file.
pub trait RecordFileFormat: Default + Send {
    /// The extension of every file, with its dot, e.g. `.xml`.
    const EXTENSION: &'static str;

    /// Appends the content of the file of `tree` to `out`, with the `annotations` it carries.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    fn write_file(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut Vec<u8>,
    ) -> io::Result<()>;
}

/// Record XML (`docs/record-xml.md`).
impl RecordFileFormat for RecordXml {
    const EXTENSION: &'static str = ".xml";

    fn write_file(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut Vec<u8>,
    ) -> io::Result<()> {
        self.write_annotated(tree, annotations, out)
    }
}

/// Record JSON (`docs/record-json.md`) in the layout of the writer, pretty by default.
impl RecordFileFormat for RecordJson {
    const EXTENSION: &'static str = ".json";

    fn write_file(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut Vec<u8>,
    ) -> io::Result<()> {
        self.write_annotated(tree, annotations, out)
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::observation::{Output, RecordXmlObserver, RecordXmlSpan};
use super::record_xml::RecordXml;
use crate::{Annotations, PositionedRecord, PositionedSources, PositionedValue};

impl RecordXml {
    /// Writes Record XML and observes the original slots and byte ranges of each emitted field.
    ///
    /// # Errors
    /// Returns a positioning, writer or observer error.
    pub fn write_positioned_observed<'tree, 'source, W: Write>(
        &mut self,
        input: &'tree PositionedRecord<'source>,
        out: &mut W,
        observe: &mut impl FnMut(
            RecordXmlSpan<'tree, 'source>,
            Option<PositionedValue<'tree, 'source>>,
            &[PositionedSources],
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        self.write_positioned_observed_annotated(input, Annotations::none(), out, observe)
    }

    /// Writes one wrapped Record element and observes its field ranges relative to the fragment.
    ///
    /// # Errors
    /// Returns a positioning, writer or observer error.
    pub fn write_wrapped_positioned_observed<'tree, 'source, W: Write>(
        &mut self,
        input: &'tree PositionedRecord<'source>,
        depth: usize,
        out: &mut W,
        observe: &mut impl FnMut(
            RecordXmlSpan<'tree, 'source>,
            Option<PositionedValue<'tree, 'source>>,
            &[PositionedSources],
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        self.write_wrapped_positioned_observed_annotated(
            input,
            Annotations::none(),
            depth,
            out,
            observe,
        )
    }

    /// Writes an annotated Record XML document and observes each emitted source-bearing field.
    /// Locale observations retain the resolver outcome that supplied their actual bytes.
    ///
    /// # Errors
    /// Returns a positioning, writer or observer error.
    pub fn write_positioned_observed_annotated<'tree, 'source, W: Write>(
        &mut self,
        input: &'tree PositionedRecord<'source>,
        annotations: Annotations<'_>,
        out: &mut W,
        observe: &mut impl RecordXmlObserver<'tree, 'source>,
    ) -> io::Result<()> {
        let mut out = Output::positioned(out, input, observe);
        self.write_document(input.tree(), annotations, &mut out)
    }

    /// Writes an annotated Record element and observes field ranges relative to that fragment.
    ///
    /// # Errors
    /// Returns a positioning, writer or observer error.
    pub fn write_wrapped_positioned_observed_annotated<'tree, 'source, W: Write>(
        &mut self,
        input: &'tree PositionedRecord<'source>,
        annotations: Annotations<'_>,
        depth: usize,
        out: &mut W,
        observe: &mut impl RecordXmlObserver<'tree, 'source>,
    ) -> io::Result<()> {
        let mut out = Output::positioned(out, input, observe);
        self.write_record(&mut out, input.tree(), annotations, depth, false)
    }
}

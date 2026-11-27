// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_cryxml::{Document, XmlWriteField};

use crate::origin_companion::{OriginArtifactBinding, OriginOutputAddress, write_origin_companion};
use crate::{
    DecimalU64, FieldOrigin, SharedFieldOrigin, SourceInventory, SourceScope, SourceUse, SourceUses,
};

/// Original decoded offset and selection evidence for embedded CryXml.
pub struct EmbeddedXml<'source> {
    base: u64,
    selectors: &'source [SourceUse],
}

impl<'source> EmbeddedXml<'source> {
    /// Creates evidence for the document at `base` in the decoded physical source.
    #[must_use]
    pub const fn new(base: u64, selectors: &'source [SourceUse]) -> Self {
        Self { base, selectors }
    }
}

struct EmbeddedEvidence {
    base: u64,
    shared: SharedFieldOrigin,
}

/// The origins of bytes written by the XML renderer.
pub struct XmlReceipt {
    inventory: SourceInventory,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl XmlReceipt {
    /// Writes unchanged XML bytes and captures their opaque payload origin.
    /// `origin` must describe `bytes` and reference uses from `uses`.
    ///
    /// # Errors
    /// Returns an error if the output or its source inventory cannot be written.
    pub fn opaque(
        writer: &mut dyn Write,
        bytes: &[u8],
        uses: &SourceUses,
        origin: FieldOrigin,
    ) -> io::Result<Self> {
        writer.write_all(bytes)?;
        Ok(Self {
            inventory: uses.freeze().map_err(io::Error::other)?,
            fields: vec![(OriginOutputAddress::OpaquePayload, origin)],
        })
    }

    /// Writes binary CryXml as XML and captures the original contributors to each field.
    /// `payload` and any embedded selectors must come from `uses`.
    ///
    /// # Errors
    /// Returns an error if the XML or its source evidence cannot be recorded.
    pub fn document(
        document: &Document<'_>,
        writer: &mut dyn Write,
        (uses, payload): (&SourceUses, &SourceScope),
        complete: bool,
        embedded: Option<&EmbeddedXml<'_>>,
    ) -> io::Result<Self> {
        let embedded = embedded.map(|origin| EmbeddedEvidence {
            base: origin.base,
            shared: uses.shared_origin(origin.selectors, complete),
        });
        let mut fields = Vec::new();
        document.write_xml_observed(writer, |field| {
            fields.try_reserve(1).map_err(io::Error::other)?;
            fields.push(binary_field(
                &field,
                uses,
                payload,
                complete,
                embedded.as_ref(),
            )?);
            Ok(())
        })?;
        Ok(Self {
            inventory: uses.freeze().map_err(io::Error::other)?,
            fields,
        })
    }

    /// Writes the source companion bound to the rendered output.
    /// The caller supplies the binding measured from the exact rendered bytes.
    ///
    /// # Errors
    /// Returns the first companion encoding or output error.
    pub fn write(&self, writer: &mut dyn Write, binding: &OriginArtifactBinding) -> io::Result<()> {
        write_origin_companion(
            writer,
            binding,
            &self.inventory,
            self.fields
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

fn binary_field(
    field: &XmlWriteField,
    uses: &SourceUses,
    payload: &SourceScope,
    complete: bool,
    embedded: Option<&EmbeddedEvidence>,
) -> io::Result<(OriginOutputAddress, FieldOrigin)> {
    let captured = embedded
        .map_or_else(
            || payload.cryxml_field(field),
            |embedded| payload.cryxml_field_at(field, embedded.base),
        )
        .map_err(io::Error::other)?
        .ok_or_else(|| io::Error::other("unknown XML writer field category"))?;
    let origin = match embedded {
        Some(origin) => uses
            .origin_with_shared(&origin.shared, &captured.uses, complete)
            .map_err(io::Error::other)?,
        None => uses.origin(&captured.uses, complete),
    };
    let range = field.range();
    let address = OriginOutputAddress::XmlBytes {
        start: DecimalU64::new(range.start),
        end_exclusive: DecimalU64::new(range.end),
        field: captured.field,
    };
    Ok((address, origin))
}

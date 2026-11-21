// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use anyhow::Result;
use p4kx_extraction::cryxml::{Document, Xml, XmlWriteField};
use p4kx_extraction::{
    DecimalU64, FieldOrigin, FieldOrigins, FieldPath, InputScope, ObservedFileRead,
    OriginArtifactBinding, OriginOutputAddress, SharedFieldOrigin, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, write_origin_companion,
};

pub(crate) struct CryXmlOrigins {
    uses: SourceUses,
    payload: SourceScope,
    complete: bool,
}

pub(crate) struct EmbeddedXml<'source> {
    pub(crate) base: u64,
    pub(crate) selectors: &'source [SourceUse],
}

struct EmbeddedEvidence {
    base: u64,
    shared: SharedFieldOrigin,
}

pub(crate) struct XmlReceipt {
    inventory: SourceInventory,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl CryXmlOrigins {
    pub(crate) fn new(read: &ObservedFileRead) -> Result<Self> {
        let uses = SourceUses::new();
        let input = read
            .origin()
            .map_or(SourceInput::Unknown, SourceInput::File);
        let payload = uses.observe(input, InputScope::Payload)?;
        Ok(Self {
            uses,
            payload,
            complete: read.origin().is_some(),
        })
    }

    pub(crate) fn summary(&self, xml: &Xml<'_>) -> Result<(FieldOrigins, SourceInventory)> {
        let mut fields = FieldOrigins::new();
        let metadata = self.payload.for_scope(InputScope::FileMetadata);
        let path = metadata.at(
            SourceLocation::Metadata {
                field: "path".to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        fields.insert(
            FieldPath::parse("/input")?,
            self.uses.origin(&[path], self.complete),
        )?;
        for pointer in ["/output", "/originReceipt"] {
            fields.insert(FieldPath::parse(pointer)?, self.uses.origin(&[], true))?;
        }
        let (namespace, format, order) = match xml {
            Xml::Text(_) => ("xml", "formatClassifier", "formatClassifier"),
            Xml::CryXml(_) => ("cryxmlHeader", "signature", "byteOrder"),
        };
        for (pointer, field) in [("/format", format), ("/byte_order", order)] {
            let field = self.payload.at(
                location(namespace, Vec::new(), field),
                SourceUseRole::Payload,
            )?;
            fields.insert(
                FieldPath::parse(pointer)?,
                self.uses.origin(&[field], self.complete),
            )?;
        }
        Ok((fields, self.uses.freeze()?))
    }

    pub(crate) fn render(&self, xml: &Xml<'_>, writer: &mut dyn Write) -> io::Result<XmlReceipt> {
        match xml {
            Xml::Text(bytes) => {
                let datum = self
                    .payload
                    .at(
                        location("xml", vec![0, bytes.len() as u64], "opaquePayload"),
                        SourceUseRole::Payload,
                    )
                    .map_err(io::Error::other)?;
                XmlReceipt::opaque(
                    writer,
                    bytes,
                    &self.uses,
                    self.uses.origin(&[datum], self.complete),
                )
            }
            Xml::CryXml(document) => XmlReceipt::document(
                document,
                writer,
                (&self.uses, &self.payload),
                self.complete,
                None,
            ),
        }
    }
}

impl XmlReceipt {
    pub(crate) fn opaque(
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

    pub(crate) fn document(
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

    pub(crate) fn write(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
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

fn location(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}

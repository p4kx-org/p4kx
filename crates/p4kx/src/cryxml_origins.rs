// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use anyhow::Result;
use p4kx_extraction::cryxml::Xml;
use p4kx_extraction::{
    FieldOrigins, FieldPath, InputScope, ObservedFileRead, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUseRole, SourceUses, XmlReceipt,
};

pub(crate) struct CryXmlOrigins {
    uses: SourceUses,
    payload: SourceScope,
    complete: bool,
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

fn location(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}

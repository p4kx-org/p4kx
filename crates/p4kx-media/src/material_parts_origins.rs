// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldPath, InputScope, MaterialAttributeInfo, MaterialAttributeKindInfo, MaterialContentInfo,
    MaterialElementRow, MaterialEncodingInfo, SourceUseRole as Role,
};

use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{child, independent};
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, SourceContext, origin_error};

pub(crate) fn payload() -> Result<FieldPath, MediaError> {
    FieldPath::parse("/data/payload").map_err(|cause| origin_error::address(&cause))
}

pub(crate) fn prepared(
    context: &SourceContext<'_>,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = writer.record_origins();
    prepared.set_builtin(BuiltinOrigins {
        profile: OriginProfile::MaterialPart,
        input: context.origin_input(writer)?,
        scope: InputScope::Payload,
    })?;
    Ok(prepared)
}

pub(crate) fn capture(
    input: &MediaOriginInput,
    scope: InputScope,
    site: Option<OriginSite<'_>>,
    role: Role,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    if let Some(site) = site {
        capture.add(input, scope, site, role, writer)?;
    } else {
        capture.add_unavailable(input, scope, role, writer)?;
    }
    Ok(capture.finish())
}

pub(super) struct ElementOrigins<'call, 'writer> {
    pub(super) input: &'call MediaOriginInput,
    pub(super) original: Option<&'call [u64]>,
    pub(super) encoding: MaterialEncodingInfo,
    pub(super) writer: &'call MediaWriter<'writer>,
}

impl ElementOrigins<'_, '_> {
    pub(super) fn element(
        &self,
        prepared: &mut PreparedRecordOrigins,
        row: &MaterialElementRow<'_>,
    ) -> Result<(), MediaError> {
        let payload = payload()?;
        let address = child(&child(&payload, "part")?, "row")?;
        independent(prepared, &payload, self.writer)?;
        match row {
            MaterialElementRow::Element { namespace, .. } => {
                self.header(prepared, &address, namespace.is_some())
            }
            MaterialElementRow::DecodedAttribute { position, .. } => {
                let proof = self.item("materialXmlAttribute", *position, "attribute")?;
                prepared.insert_subtree(&child(&address, "value")?, proof, self.writer)
            }
            MaterialElementRow::Declaration {
                position, value, ..
            } => self.declaration(prepared, &address, *position, value),
            MaterialElementRow::Content {
                position, value, ..
            } => {
                let (position, namespace) = match (self.encoding, value) {
                    (MaterialEncodingInfo::CryXml, MaterialContentInfo::Element { ordinal }) => {
                        (*ordinal, "materialXmlElement")
                    }
                    _ => (*position, "materialXmlContent"),
                };
                let proof = self.item(namespace, position, "content")?;
                prepared.insert_subtree(&child(&address, "value")?, proof, self.writer)
            }
            _ => Err(MediaError::Contract(
                "unsupported material element row".into(),
            )),
        }
    }

    fn header(
        &self,
        prepared: &mut PreparedRecordOrigins,
        address: &FieldPath,
        has_namespace: bool,
    ) -> Result<(), MediaError> {
        let source_fields: &[(&[&str], _, _)] = &[
            (&["name", "qualified_name"], "qualifiedName", Role::Payload),
            (&["ordinal"], "childOrdinal", Role::Selector),
            (&["binary_text"], "binaryText", Role::Payload),
            (
                &["attributes", "declarations", "contents", "children"],
                "retainedChildren",
                Role::Conversion,
            ),
        ];
        for (fields, field, role) in source_fields {
            let proof = self.at("materialXmlElement", &[], field, *role)?;
            for name in *fields {
                prepared.insert_subtree(&child(address, name)?, proof.clone(), self.writer)?;
            }
        }
        let namespace = if has_namespace {
            capture(
                self.input,
                InputScope::Payload,
                None,
                Role::Conversion,
                self.writer,
            )?
        } else {
            self.at("materialXmlElement", &[], "namespace", Role::Selector)?
        };
        prepared.insert_subtree(&child(address, "namespace")?, namespace, self.writer)
    }

    fn declaration(
        &self,
        prepared: &mut PreparedRecordOrigins,
        address: &FieldPath,
        position: usize,
        value: &MaterialAttributeInfo<'_>,
    ) -> Result<(), MediaError> {
        let proof = self.item("materialXmlDeclaration", position, "declaration")?;
        let value_path = child(address, "value")?;
        prepared.insert_subtree(&value_path, proof, self.writer)?;
        if value.lexeme.is_none() {
            let mut proof = self.writer.capture_origins();
            proof.include_original(&self.at(
                "materialXmlDeclaration",
                &[crate::shader_values::count(position)?],
                "lexeme",
                Role::Conversion,
            )?)?;
            proof.include_original(&capture(
                self.input,
                InputScope::Payload,
                Some(OriginSite::new("materialXmlDocument", &[], "encoding")),
                Role::Selector,
                self.writer,
            )?)?;
            prepared.insert_subtree(&child(&value_path, "lexeme")?, proof.finish(), self.writer)?;
        }
        if matches!(
            &value.kind,
            MaterialAttributeKindInfo::Attribute { namespace: Some(_) }
        ) {
            let resolved = capture(
                self.input,
                InputScope::Payload,
                None,
                Role::Conversion,
                self.writer,
            )?;
            prepared.insert_subtree(
                &child(&child(&value_path, "kind")?, "namespace")?,
                resolved,
                self.writer,
            )?;
        }
        Ok(())
    }

    fn item(
        &self,
        namespace: &str,
        position: usize,
        field: &str,
    ) -> Result<OriginContributions, MediaError> {
        self.at(
            namespace,
            &[crate::shader_values::count(position)?],
            field,
            Role::Payload,
        )
    }

    fn at(
        &self,
        namespace: &str,
        suffix: &[u64],
        field: &str,
        role: Role,
    ) -> Result<OriginContributions, MediaError> {
        let site = self
            .original
            .map(|path| OriginSite::new(namespace, suffix, field).prefixed(path));
        capture(self.input, InputScope::Payload, site, role, self.writer)
    }
}

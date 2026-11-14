// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ChunkDirectory;
use p4kx_extraction::model::MaterialName;
use p4kx_extraction::{ArchiveMaterialPaths, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::material_path;
use super::origins::{self, ModelOrigins};
use super::payload::ModelSourceFamily;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::required_reader::RequiredReader;
use crate::{
    MediaError, MediaWriter, OffsetBasis, OriginalIdentifier, Reference, ReferenceUnavailable,
    SourceContext, SourceRange, Status, Subresource,
};

const NAMESPACE: &str = "model-material-library";
const SCHEMA: &str = "ModelMaterialLibraryRecord";
const REFERENCE: &str = "/data/payload/reference";
pub(super) const SCHEMA_ROW: (&str, &str, usize, &str) = (NAMESPACE, "material-library", 1, SCHEMA);

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum Record {
    MaterialLibrary(Payload),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
enum Payload {
    Named {
        primary_model_reader_selector: bool,
        token: SourceRange,
        #[schemars(schema_with = "crate::schema::reference_definition")]
        reference: Box<Reference>,
    },
    Empty {
        primary_model_reader_selector: bool,
        field: SourceRange,
    },
    NonUtf8 {
        primary_model_reader_selector: bool,
        token: SourceRange,
    },
    UnsupportedVersion,
}

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), MediaError> {
    crate::schema_merge::insert_payload(
        base,
        SCHEMA,
        crate::schema_merge::serialized_payload::<Record>(),
    )
}

pub(super) struct Source<'context, 'input> {
    context: &'context SourceContext<'input>,
    primary: Option<usize>,
}

impl<'context, 'input> Source<'context, 'input> {
    pub(super) fn new(
        family: ModelSourceFamily,
        file: &ChunkDirectory,
        context: &'context SourceContext<'input>,
    ) -> Self {
        let primary = if family.is_model() {
            MaterialName::primary(file.chunks())
        } else {
            None
        };
        Self { context, primary }
    }

    pub(super) fn emit(
        &self,
        file: &ChunkDirectory,
        ordinal: usize,
        reader: &mut RequiredReader<'_>,
        origins: &ModelOrigins<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let missing = || MediaError::Contract("model material selector lost its chunk".into());
        let chunk = file.chunks().get(ordinal).ok_or_else(missing)?;
        let row = file.table_row(ordinal).ok_or_else(missing)?;
        let mut prepared = origins.prepared()?;
        let mut layout = writer.capture_origins();
        for (range, name) in [
            (row.kind, "modelMaterialNameKind"),
            (row.version, "modelMaterialNameVersion"),
        ] {
            layout.include_original(&origins.payload(
                range,
                name,
                SourceUseRole::Selector,
                writer,
            )?)?;
        }
        let layout = layout.finish();
        origins::at(
            &mut prepared,
            "/subresource/indices",
            layout.clone(),
            writer,
        )?;
        origins::at(&mut prepared, "/data/payload", layout, writer)?;

        let payload = if MaterialName::supports(chunk) {
            primary_origin(file, origins, &mut prepared, writer)?;
            let start = chunk.offset();
            let field = start..start + MaterialName::prefix_len(chunk.size());
            let bytes = reader.read_chunk(field.clone())?;
            let token_bytes = MaterialName::bytes(&bytes);
            let metadata = NameField {
                field: SourceRange::new(OffsetBasis::Entry, field.start, field.end),
                token: SourceRange::new(
                    OffsetBasis::Entry,
                    start,
                    start + token_bytes.len() as u64,
                ),
                bytes: token_bytes,
                primary_model_reader_selector: self.primary == Some(ordinal),
            };
            self.payload(metadata, origins, &mut prepared, writer)?
        } else {
            Payload::UnsupportedVersion
        };
        prepared.write(
            writer,
            &Subresource::new(NAMESPACE, vec![ordinal as u64]),
            Status::Complete,
            &Record::MaterialLibrary(payload),
            &[],
        )
    }

    fn payload(
        &self,
        field: NameField<'_>,
        origins: &ModelOrigins<'_>,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Payload, MediaError> {
        let primary_model_reader_selector = field.primary_model_reader_selector;
        let original = std::str::from_utf8(field.bytes);
        if original.is_ok_and(|name| name.trim().is_empty()) {
            let evidence = name_origin(&field.field, origins, writer)?;
            origins::at(prepared, "/data/payload/field", evidence.clone(), writer)?;
            origins::at(prepared, "/data/payload/disposition", evidence, writer)?;
            return Ok(Payload::Empty {
                primary_model_reader_selector,
                field: field.field,
            });
        }
        let evidence = name_origin(&field.token, origins, writer)?;
        origins::at(prepared, "/data/payload/token", evidence.clone(), writer)?;
        origins::at(
            prepared,
            "/data/payload/disposition",
            evidence.clone(),
            writer,
        )?;
        let Ok(name) = original else {
            return Ok(Payload::NonUtf8 {
                primary_model_reader_selector,
                token: field.token,
            });
        };
        let mut selector = writer.capture_origins();
        selector.include_original(&evidence)?;
        let paths = ArchiveMaterialPaths::new(self.context.entry.path(), name.trim());
        if paths.as_ref().is_ok_and(|paths| paths.sibling.is_some()) {
            selector.include_original(&origins.directory("modelMaterialSiblingFolder", writer)?)?;
        }
        let original = OriginalIdentifier::new_with_path(name);
        let observed = material_path::observe(
            self.context.directory,
            original,
            paths.map_err(|_| ReferenceUnavailable::InvalidIdentifier),
            writer,
        )?;
        observed.write(REFERENCE, &evidence, &selector.finish(), prepared, writer)?;
        Ok(Payload::Named {
            primary_model_reader_selector,
            token: field.token,
            reference: Box::new(observed.reference),
        })
    }
}

fn primary_origin(
    file: &ChunkDirectory,
    origins: &ModelOrigins<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let primary = origins.payload(
        file.table_range(),
        "modelMaterialPrimarySelection",
        SourceUseRole::Selector,
        writer,
    )?;
    let mut selection = writer.capture_origins();
    selection.include_original(&primary)?;
    selection.include_original(&origins.directory("modelExtension", writer)?)?;
    origins::at(
        prepared,
        "/data/payload/primary_model_reader_selector",
        selection.finish(),
        writer,
    )
}

struct NameField<'a> {
    field: SourceRange,
    token: SourceRange,
    bytes: &'a [u8],
    primary_model_reader_selector: bool,
}

fn name_origin(
    range: &SourceRange,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    origins.payload(
        range.start..range.end_exclusive,
        "modelMaterialName",
        SourceUseRole::Payload,
        writer,
    )
}

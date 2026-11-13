// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{Format, Signature};
use p4kx_extraction::model::character_definition::CharacterDefinition;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::Serialize;

use crate::Reference;

pub(super) const DOCUMENT: &str = "model-character-definition";
pub(super) const SKELETON: &str = "model-character-skeleton";
pub(super) const ATTACHMENT: &str = "model-character-attachment";

#[derive(Serialize, JsonSchema)]
enum CharacterEncoding {
    #[serde(rename = "text-xml")]
    TextXml,
    CryXmlB,
    CryXml,
    #[serde(rename = "CRY3SDK")]
    Cry3Sdk,
}

impl From<Format> for CharacterEncoding {
    fn from(value: Format) -> Self {
        match value {
            Format::Text => Self::TextXml,
            Format::CryXml(Signature::CryXmlB) => Self::CryXmlB,
            Format::CryXml(Signature::CryXml) => Self::CryXml,
            Format::CryXml(Signature::Cry3Sdk) => Self::Cry3Sdk,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Definition {
    encoding: Option<CharacterEncoding>,
    root_element: Option<String>,
    disposition: CharacterDefinitionDisposition,
    strings_lossy: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum CharacterDefinitionDisposition {
    CharacterDefinition,
    OtherRoot,
    Malformed,
}

impl Definition {
    pub(super) fn new(parsed: Option<&CharacterDefinition<'_>>) -> Self {
        let disposition = match parsed {
            Some(source) if source.is_character_definition() => {
                CharacterDefinitionDisposition::CharacterDefinition
            }
            Some(_) => CharacterDefinitionDisposition::OtherRoot,
            None => CharacterDefinitionDisposition::Malformed,
        };
        Self {
            encoding: parsed.map(|source| source.format().into()),
            root_element: parsed.map(|source| source.root().name().to_owned()),
            disposition,
            strings_lossy: parsed.map(CharacterDefinition::is_lossy),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Skeleton {
    #[schemars(schema_with = "optional_reference")]
    pub(super) file: Option<Reference>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct BoneOffset<T> {
    pub(super) authored: Option<String>,
    pub(super) value: Option<T>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Attachment {
    pub(super) name: Option<String>,
    #[serde(rename = "attachment_type")]
    pub(super) kind: Option<String>,
    #[schemars(schema_with = "optional_reference")]
    pub(super) binding: Option<Reference>,
    pub(super) material: Option<String>,
    pub(super) bone_name: Option<String>,
    pub(super) relative_position: BoneOffset<[f32; 3]>,
    pub(super) relative_rotation_wxyz: BoneOffset<[f32; 4]>,
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum CharacterRecord {
    #[serde(rename = "character-definition")]
    Definition(Definition),
    #[serde(rename = "character-skeleton")]
    Skeleton(Skeleton),
    #[serde(rename = "character-attachment")]
    Attachment(Attachment),
}

fn optional_reference(_: &mut SchemaGenerator) -> Schema {
    json_schema!({"anyOf": [{"$ref": "#/$defs/Reference"}, {"type": "null"}]})
}

const SCHEMA_NAME: &str = "ModelCharacterRecord";
pub(super) const SCHEMA_ROWS: [(&str, &str, usize, &str); 3] = [
    (DOCUMENT, "character-definition", 0, SCHEMA_NAME),
    (SKELETON, "character-skeleton", 1, SCHEMA_NAME),
    (ATTACHMENT, "character-attachment", 2, SCHEMA_NAME),
];

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), crate::MediaError> {
    crate::schema_merge::insert_payload(
        base,
        SCHEMA_NAME,
        crate::schema_merge::serialized_payload::<CharacterRecord>(),
    )
}

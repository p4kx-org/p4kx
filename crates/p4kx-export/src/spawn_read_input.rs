// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, Value};

use crate::guid_text::CatalogGuid;
use crate::record_fields;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldChecks, ReferenceShape, unknown_struct};
use crate::spawn_read::Reader;
use crate::spawn_tag_text::SpawnTagText;
use crate::{CatalogError, ExportAccountingError};
#[path = "spawn_structs.rs"]
mod structs;

/// Actual source accesses made by the existing spawn-content algorithms.
pub(crate) trait ReadContext<'source> {
    fn capture(&self) -> &dyn FieldCapture<'source>;
    fn check_tree(&self, tree: &RecordTree<'source>) -> Result<(), ReadError>;
    fn field(&self, parent: &StructValue<'source>, name: &str) -> Result<(), ReadError>;
    fn scalar(&self, value: &Value<'source>) -> Result<(), ReadError>;
    fn element(&self, value: &Value<'source>) -> Result<(), ReadError>;
    fn source_type(&self, source: &StructValue<'source>) -> Result<(), ReadError>;
    fn tag(&self, value: &Value<'source>) -> Result<Option<SpawnTagText>, ReadError>;
    fn variable(&self, value: &Value<'source>) -> Result<Option<String>, ReadError>;
    fn authored_reference(&self, value: &Value<'source>) -> Result<(), ReadError>;
    fn record_guid(&self, value: &Value<'source>) -> Result<Option<CatalogGuid>, ReadError>;
}

/// A fallible access adapter over the one existing reader and an actual source Struct.
pub(crate) struct ReadFields<'reader, 'parent, 'source> {
    reader: &'reader Reader<'reader, 'source>,
    parent: Option<&'parent StructValue<'source>>,
}

impl<'reader, 'parent, 'source> ReadFields<'reader, 'parent, 'source> {
    pub(super) const fn new(
        reader: &'reader Reader<'reader, 'source>,
        parent: Option<&'parent StructValue<'source>>,
    ) -> Self {
        Self { reader, parent }
    }

    fn context(&self) -> Option<&'reader dyn ReadContext<'source>> {
        self.reader.context
    }

    pub(crate) fn get(&self, name: &str) -> Result<Option<&'parent Value<'source>>, ReadError> {
        let Some(parent) = self.parent else {
            return Ok(None);
        };
        if let Some(capture) = self.reader.capture() {
            capture.check_parent(parent)?;
        }
        if let Some(context) = self.context() {
            context.field(parent, name)?;
        }
        Ok(parent.field(name))
    }

    fn scalar(&self, name: &str) -> Result<Option<&'parent Value<'source>>, ReadError> {
        let value = self.get(name)?;
        if let (Some(context), Some(value)) = (self.context(), value) {
            context.scalar(value)?;
        }
        Ok(value)
    }

    fn container(&self, name: &str) -> Result<Option<&'parent Value<'source>>, ReadError> {
        let value = self.get(name)?;
        if let Some(value) = value {
            self.element(value)?;
        }
        Ok(value)
    }

    pub(crate) fn boolean(&self, name: &str) -> Result<Option<bool>, ReadError> {
        Ok(match self.scalar(name)? {
            Some(Value::Bool(value)) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn integer(&self, name: &str) -> Result<Option<i64>, ReadError> {
        Ok(self.scalar(name)?.and_then(record_fields::integer_value))
    }

    pub(crate) fn number(&self, name: &str) -> Result<Option<f32>, ReadError> {
        Ok(self.scalar(name)?.and_then(record_fields::number_value))
    }

    pub(crate) fn text(&self, name: &str) -> Result<Option<&'source str>, ReadError> {
        Ok(match self.scalar(name)? {
            Some(Value::String(value) | Value::Enum(value)) if !value.is_empty() => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn owned(&self, name: &str) -> Result<Option<String>, ReadError> {
        Ok(self.text(name)?.map(str::to_owned))
    }

    pub(crate) fn key(&self, name: &str) -> Result<Option<&'source str>, ReadError> {
        Ok(self
            .scalar(name)?
            .and_then(crate::mission_layers::key_value))
    }

    pub(crate) fn strict_number(&self, name: &str, path: &str) -> Result<Option<f32>, ReadError> {
        self.scalar(name)?;
        let Some(parent) = self.parent else {
            return Ok(None);
        };
        FieldChecks::for_context(parent, path, self.reader.capture())?.number(name)
    }

    pub(crate) fn strict_reference(
        &self,
        name: &str,
        path: &str,
    ) -> Result<Option<crate::definition_reference::DefinitionReference>, ReadError> {
        let value = self.get(name)?;
        let Some(parent) = self.parent else {
            return Ok(None);
        };
        let result = FieldChecks::for_context(parent, path, self.reader.capture())?
            .reference_view(name)?
            .map(ReferenceShape::into_owned);
        if let (Some(context), Some(value)) = (self.context(), value) {
            context.authored_reference(value)?;
        }
        Ok(result)
    }

    pub(crate) fn reference(&self, name: &str) -> Result<Option<CatalogGuid>, ReadError> {
        self.get(name)?
            .map(|value| self.record_guid(value))
            .transpose()
            .map(Option::flatten)
    }

    pub(crate) fn references(&self, name: &str) -> Result<Vec<CatalogGuid>, ReadError> {
        let Some(Value::Array(array)) = self.container(name)? else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for value in array {
            if let Some(guid) = self.record_guid(value)? {
                found.push(guid);
            }
        }
        Ok(found)
    }

    fn record_guid(&self, value: &Value<'source>) -> Result<Option<CatalogGuid>, ReadError> {
        self.context().map_or_else(
            || Ok(record_fields::record_reference(value)),
            |context| context.record_guid(value),
        )
    }

    pub(crate) fn items(&self, name: &str) -> Result<&'parent [Value<'source>], ReadError> {
        Ok(match self.container(name)? {
            Some(Value::Array(items)) => items,
            Some(single @ Value::Struct(_)) => std::slice::from_ref(single),
            _ => &[],
        })
    }

    pub(crate) fn one(
        &self,
        name: &str,
    ) -> Result<Option<&'parent StructValue<'source>>, ReadError> {
        for value in self.items(name)? {
            self.element(value)?;
            if let Value::Struct(source) = value {
                return Ok(Some(source));
            }
        }
        Ok(None)
    }

    pub(crate) fn structs(
        &self,
        name: &str,
    ) -> Result<
        impl Iterator<Item = Result<&'parent StructValue<'source>, ReadError>>
        + use<'reader, 'parent, 'source>,
        ReadError,
    > {
        Ok(structs::Structs::new(self.items(name)?, self.context()))
    }

    pub(crate) fn tag_list(&self, name: &str) -> Result<Vec<SpawnTagText>, ReadError> {
        self.reader.fields(self.one(name)?).tag_array("tags")
    }

    pub(crate) fn tag_array(&self, name: &str) -> Result<Vec<SpawnTagText>, ReadError> {
        let mut paths = Vec::new();
        for value in self.items(name)? {
            let path = self.context().map_or_else(
                || Ok(self.reader.path(value).map(SpawnTagText::owned)),
                |context| context.tag(value),
            )?;
            if let Some(path) = path {
                paths.push(path);
            }
        }
        Ok(paths)
    }

    pub(crate) fn variable(&self, name: &str) -> Result<Option<String>, ReadError> {
        let Some(value) = self.get(name)? else {
            return Ok(None);
        };
        self.context().map_or_else(
            || Ok(self.reader.variable_of(value)),
            |context| context.variable(value),
        )
    }

    pub(crate) fn unknown_type(&self, operation: &'static str) -> ReadError {
        match self.parent {
            Some(parent) => unknown_struct(parent, operation, self.reader.capture()),
            None => Box::new(CatalogError::from(
                ExportAccountingError::ConflictingSourceDatum,
            )),
        }
    }

    pub(crate) fn type_name(&self) -> Result<&'source str, ReadError> {
        let parent = self.parent.ok_or_else(|| {
            Box::new(CatalogError::from(
                ExportAccountingError::ConflictingSourceDatum,
            )) as ReadError
        })?;
        if let Some(capture) = self.reader.capture() {
            capture.check_parent(parent)?;
        }
        if let Some(context) = self.context() {
            context.source_type(parent)?;
        }
        Ok(parent.type_name())
    }

    fn element(&self, value: &Value<'source>) -> Result<(), ReadError> {
        if let Some(context) = self.context() {
            context.element(value)?;
        }
        Ok(())
    }
}

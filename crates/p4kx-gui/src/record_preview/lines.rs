// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;

use p4kx_extraction::{
    DataForge, Field, RecordTree, RecordXml, Reference, Resolution, TAG_TYPE, TagResolver,
    TextResolver, Value, ValueError,
};

use super::xml_provenance::PendingXml;
use super::{FieldLine, KeyText, LineValue, RecordLink, RecordView};
use crate::model::describe;
use crate::shown_line::display_text;
use crate::{RecordId, XmlDocument};

/// Builds the preview of the Record `tree` of `forge`, resolving each Reference to the Record it
/// selects, each tag Reference to its tag path through `tags` and, with `texts`, each
/// localisation key to its text.
pub(super) fn view(
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
    texts: Option<&dyn TextResolver>,
    tags: &dyn TagResolver,
) -> (RecordView, Option<PendingXml>) {
    let mut lines = Lines {
        forge,
        texts,
        tags,
        out: Vec::new(),
        text: String::new(),
    };
    lines.fields(tree.fields(), 0);
    let mut xml = Vec::new();
    let mut observed = PendingXml::new();
    let xml_error = RecordXml::new()
        .write_observed(tree, texts.into(), &mut xml, &mut |span| {
            observed.observe(span);
            Ok(())
        })
        .err()
        .map(|error| describe(&error).into_boxed_str());
    if xml_error.is_some() {
        xml.clear();
    }
    lines.out.shrink_to_fit();
    let proof = xml_error.is_none().then_some(observed);
    let view = RecordView {
        name: tree.name().into(),
        type_name: tree.type_name().into(),
        guid: tree.guid(),
        record_path: tree.record_path().into(),
        lines: lines.out,
        errors: tree.error_count(),
        xml: XmlDocument::new(xml, false),
        xml_error,
        language: None,
        texts_failure: None,
        provenance: None,
    };
    (view, proof)
}

struct Lines<'a, 'f> {
    forge: &'a DataForge<'f>,
    texts: Option<&'a dyn TextResolver>,
    tags: &'a dyn TagResolver,
    out: Vec<FieldLine>,
    text: String,
}

impl Lines<'_, '_> {
    fn fields(&mut self, fields: &[Field<'_>], depth: u16) {
        for field in fields {
            self.value(field.name().into(), field.value(), depth);
        }
    }

    fn value(&mut self, name: Box<str>, value: &Value<'_>, depth: u16) {
        let deeper = depth.saturating_add(1);
        let shown = match value {
            Value::Struct(structure) => LineValue::Struct(structure.type_name().into()),
            Value::Array(items) => LineValue::Array(items.len()),
            Value::Null => LineValue::Null,
            Value::Reference(reference) => LineValue::Reference(self.link(reference)),
            Value::Locale(key) => self.key(key),
            Value::Error(ValueError::DanglingReference { guid }) => LineValue::Dangling {
                guid: *guid,
                reason: self.shown(value_error(value)),
            },
            Value::Error(_) => LineValue::Error(self.shown(value_error(value))),
            scalar => {
                self.text.clear();
                let _infallible = write!(self.text, "{scalar}");
                LineValue::Scalar(self.text.as_str().into())
            }
        };
        self.out.push(FieldLine {
            depth,
            name,
            value: shown,
        });
        match value {
            Value::Struct(structure) => self.fields(structure.fields(), deeper),
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.text.clear();
                    let _infallible = write!(self.text, "[{index}]");
                    let name = self.text.as_str().into();
                    self.value(name, item, deeper);
                }
            }
            _ => {}
        }
    }

    fn shown(&mut self, error: Option<&ValueError>) -> Box<str> {
        self.text.clear();
        if let Some(error) = error {
            let _infallible = write!(self.text, "{error}");
        }
        self.text.as_str().into()
    }

    fn key(&self, key: &str) -> LineValue {
        let text = match self.texts {
            Some(texts) if !key.is_empty() => match texts.resolve(key) {
                Resolution::Text(text) => {
                    let shown = display_text(&text);
                    KeyText::Text {
                        escapes: shown.escapes().into(),
                        text: shown.text.into(),
                    }
                }
                Resolution::Missing => KeyText::Missing,
                _ => KeyText::None,
            },
            _ => KeyText::None,
        };
        LineValue::Key {
            key: display_text(key).text.into(),
            text,
        }
    }

    fn link(&self, reference: &Reference<'_>) -> RecordLink {
        RecordLink {
            guid: reference.guid(),
            record_path: reference.record_path().into(),
            type_name: reference.type_name().into(),
            at: reference.location().map(|at| at.as_str().into()),
            tag: (reference.type_name() == TAG_TYPE)
                .then(|| self.tags.tag_path(reference.guid()))
                .flatten()
                .map(Into::into),
            target: self
                .forge
                .record_by_guid(reference.guid())
                .map(|record| RecordId(record.index())),
        }
    }
}

const fn value_error<'v>(value: &'v Value<'_>) -> Option<&'v ValueError> {
    match value {
        Value::Error(error) => Some(error),
        _ => None,
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;
use std::ops::Range;

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceDeclaration, SourceDeclarationKind, SourceElement,
    SourceElementPosition,
};
use p4kx_media::MediaRecord;
use serde_json::{Value, json};

use super::{TestResult, named, require};

#[derive(Default)]
pub(super) struct ExpectedEvents<'source> {
    timelines: VecDeque<SourceElement<'source>>,
    events: VecDeque<SourceElement<'source>>,
    attributes: VecDeque<SourceDeclaration<'source>>,
}

impl<'source> ExpectedEvents<'source> {
    pub(super) fn new(root: SourceElement<'source>) -> TestResult<Self> {
        let mut expected = Self::default();
        if !named(root, "anim_event_list") {
            return Ok(expected);
        }
        for timeline in root
            .elements()
            .filter(|element| named(*element, "animation"))
        {
            expected.timelines.push_back(timeline);
            for event in timeline
                .elements()
                .filter(|element| named(*element, "event"))
            {
                expected.events.push_back(event);
                for declaration in event.syntax()?.declarations() {
                    let declaration = declaration?;
                    if matches!(declaration.kind(), SourceDeclarationKind::Attribute { .. }) {
                        expected.attributes.push_back(declaration);
                    }
                }
            }
        }
        Ok(expected)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.timelines.is_empty() && self.events.is_empty() && self.attributes.is_empty()
    }

    pub(super) fn timeline(&mut self, row: &MediaRecord) -> TestResult {
        let original = self
            .timelines
            .pop_front()
            .ok_or("extra animation event timeline")?;
        verify_position(row, &element_position(&original.position())?)?;
        let mut names = original
            .attributes()
            .filter(|attribute| attribute.namespace().is_none() && attribute.name() == "name");
        let target = row
            .data
            .pointer("/payload/target")
            .ok_or("missing timeline target")?;
        match (names.next(), names.next()) {
            (Some(name), None) => {
                require(
                    target.pointer("/state").and_then(Value::as_str) == Some("path"),
                    "authored target state",
                )?;
                require(
                    target.pointer("/authored_path").and_then(Value::as_str) == Some(name.value()),
                    "exact original timeline name",
                )?;
                require(
                    target.pointer("/position") == Some(&attribute_position(name.position())?),
                    "timeline selector source position",
                )
            }
            (Some(_), Some(_)) => require(
                target == &json!({"state":"ambiguous-name"}),
                "duplicate names remain ambiguous",
            ),
            (None, _) => require(
                target == &json!({"state":"missing-name"}),
                "missing original target remains explicit",
            ),
        }
    }

    pub(super) fn event(&mut self, row: &MediaRecord) -> TestResult {
        let original = self.events.pop_front().ok_or("extra animation event")?;
        verify_position(row, &element_position(&original.position())?)
    }

    pub(super) fn attribute(&mut self, row: &MediaRecord) -> TestResult {
        let original = self.attributes.pop_front().ok_or("extra event attribute")?;
        let SourceDeclarationKind::Attribute { namespace } = original.kind() else {
            return Err("an original event attribute must have attribute syntax".into());
        };
        verify_position(row, &attribute_position(original.position())?)?;
        for (field, expected) in [
            ("qualified_name", Some(original.qualified_name())),
            ("namespace", namespace),
            ("value", Some(original.value())),
            ("lexeme", original.lexeme()),
        ] {
            let actual = row
                .data
                .pointer(&format!("/payload/{field}"))
                .ok_or("missing original event field")?;
            require(
                expected.map_or_else(|| actual.is_null(), |text| actual.as_str() == Some(text)),
                "original animation event field matches exactly",
            )?;
        }
        Ok(())
    }
}

fn verify_position(row: &MediaRecord, expected: &Value) -> TestResult {
    require(
        row.data.pointer("/payload/position") == Some(expected),
        "exact original event source coordinate",
    )
}

fn element_position(position: &SourceElementPosition) -> TestResult<Value> {
    match position {
        SourceElementPosition::Text(range) => Ok(text_position(range)),
        SourceElementPosition::CryXml(index) => {
            Ok(json!({"type":"cry-xml-node","node_index":index}))
        }
        _ => Err("unknown original event element coordinate".into()),
    }
}

fn attribute_position(position: &SourceAttributePosition) -> TestResult<Value> {
    match position {
        SourceAttributePosition::Text(range) => Ok(text_position(range)),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
            ..
        } => Ok(json!({
            "type":"cry-xml-attribute", "node_index":node_index, "attribute_index":attribute_index,
        })),
        _ => Err("unknown original event attribute coordinate".into()),
    }
}

fn text_position(range: &Range<usize>) -> Value {
    json!({"type":"text","start":range.start.to_string(),"end_exclusive":range.end.to_string()})
}

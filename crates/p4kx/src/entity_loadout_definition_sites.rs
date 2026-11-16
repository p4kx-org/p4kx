// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ptr;

use p4kx_assembly::{PortDefinition, PortDefinitionPosition};
use p4kx_extraction::{
    DataForge, RecordJsonLocation, SourceLocation, SourceUseRole, StructValue, Value,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct DefinitionKey {
    record: u32,
    component: usize,
    port: usize,
}

impl DefinitionKey {
    pub(super) const fn of(position: PortDefinitionPosition) -> Self {
        Self {
            record: position.record_index(),
            component: position.component_ordinal(),
            port: position.port_ordinal(),
        }
    }

    pub(super) const fn record(self) -> u32 {
        self.record
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Site {
    Value(usize),
    Struct(usize),
}

impl Site {
    pub(super) fn value(value: &Value<'_>) -> Self {
        Self::Value(ptr::from_ref(value).addr())
    }

    pub(super) fn structure(structure: &StructValue<'_>) -> Self {
        Self::Struct(ptr::from_ref(structure).addr())
    }
}

#[derive(Clone)]
struct Input {
    site: Site,
    role: SourceUseRole,
    text: Option<SourceLocation>,
    complete: bool,
}

impl Input {
    fn structure(source: &StructValue<'_>) -> Self {
        Self {
            site: Site::structure(source),
            role: SourceUseRole::Selector,
            text: None,
            complete: true,
        }
    }

    fn value(forge: &DataForge<'_>, value: &Value<'_>, role: SourceUseRole) -> Self {
        let (text, complete) = match value {
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                let location = RecordJsonLocation::borrowed_text(forge, text);
                let complete = location.is_some();
                (location, complete)
            }
            Value::Error(_) => (None, false),
            _ => (None, true),
        };
        Self {
            site: Site::value(value),
            role,
            text,
            complete,
        }
    }
}

struct PendingMember {
    path: String,
    inputs: Vec<Input>,
}

pub(super) struct PendingDefinition {
    key: DefinitionKey,
    members: Vec<PendingMember>,
}

pub(super) struct Definition {
    pub(super) members: Vec<Member>,
}

pub(super) struct Member {
    pub(super) path: String,
    pub(super) inputs: Vec<Contribution>,
    pub(super) complete: bool,
}

pub(super) struct Contribution {
    pub(super) ordinal: u64,
    pub(super) role: SourceUseRole,
    pub(super) text: Option<SourceLocation>,
}

impl PendingDefinition {
    pub(super) fn read(
        forge: &DataForge<'_>,
        definition: &PortDefinition,
        original: &StructValue<'_>,
    ) -> Self {
        let key = DefinitionKey::of(definition.position());
        let mut capture = Capture {
            forge,
            members: Vec::new(),
        };
        let name = capture.field(original, "Name", SourceUseRole::Selector);
        capture.push("", name);
        for (wire, field, role) in [
            ("display_name", "DisplayName", SourceUseRole::Payload),
            ("flags", "Flags", SourceUseRole::Payload),
            ("min_size", "MinSize", SourceUseRole::Conversion),
            ("max_size", "MaxSize", SourceUseRole::Conversion),
        ] {
            let inputs = capture.field(original, field, role);
            capture.push(format!("/{wire}"), inputs);
        }
        capture.types(original);
        capture.transform(original, definition);
        Self {
            key,
            members: capture.members,
        }
    }

    pub(super) fn sites(&self) -> impl Iterator<Item = Site> + '_ {
        self.members
            .iter()
            .flat_map(|member| member.inputs.iter().map(|input| input.site))
    }

    pub(super) fn bind(self, found: &HashMap<Site, u64>) -> (DefinitionKey, Definition) {
        let members = self
            .members
            .into_iter()
            .map(|member| {
                let mut complete = true;
                let mut inputs = Vec::new();
                for input in member.inputs {
                    complete &= input.complete;
                    if let Some(&ordinal) = found.get(&input.site) {
                        inputs.push(Contribution {
                            ordinal,
                            role: input.role,
                            text: input.text,
                        });
                    } else {
                        complete = false;
                    }
                }
                Member {
                    path: member.path,
                    inputs,
                    complete,
                }
            })
            .collect();
        (self.key, Definition { members })
    }
}

struct Capture<'a> {
    forge: &'a DataForge<'a>,
    members: Vec<PendingMember>,
}

impl Capture<'_> {
    fn push(&mut self, path: impl Into<String>, inputs: Vec<Input>) {
        self.members.push(PendingMember {
            path: path.into(),
            inputs,
        });
    }

    fn field(&self, owner: &StructValue<'_>, name: &str, role: SourceUseRole) -> Vec<Input> {
        let mut inputs = vec![Input::structure(owner)];
        if let Some(datum) = owner.field(name) {
            inputs.push(Input::value(self.forge, datum, role));
        }
        inputs
    }

    fn types(&mut self, original: &StructValue<'_>) {
        let field = "Types";
        let mut collection = self.field(original, field, SourceUseRole::Selector);
        if let Some(Value::Array(values)) = original.field(field) {
            let mut emitted = 0;
            for value in values {
                let selection = Input::value(self.forge, value, SourceUseRole::Selector);
                collection.push(selection.clone());
                if let Value::Struct(kind) = value {
                    let path = format!("/types/{emitted}");
                    self.push(path.clone(), vec![selection]);
                    let original_type = self.field(kind, "Type", SourceUseRole::Payload);
                    self.push(format!("{path}/type"), original_type);
                    self.subtypes(kind, &path);
                    emitted += 1;
                }
            }
        }
        self.push("/types", collection);
    }

    fn subtypes(&mut self, original: &StructValue<'_>, path: &str) {
        let field = "SubTypes";
        let mut collection = self.field(original, field, SourceUseRole::Selector);
        if let Some(Value::Array(values)) = original.field(field) {
            let mut emitted = 0;
            for value in values {
                collection.push(Input::value(self.forge, value, SourceUseRole::Selector));
                if matches!(value, Value::String(text) | Value::Locale(text) | Value::Enum(text) if !text.is_empty())
                {
                    let input = Input::value(self.forge, value, SourceUseRole::Payload);
                    self.push(format!("{path}/sub_types/{emitted}"), vec![input]);
                    emitted += 1;
                }
            }
        }
        self.push(format!("{path}/sub_types"), collection);
    }

    fn transform(&mut self, original: &StructValue<'_>, definition: &PortDefinition) {
        let mut inputs = Vec::new();
        let attachment = self.child(original, "AttachmentImplementation", &mut inputs);
        let helper = attachment
            .and_then(|attachment| self.child(attachment, "Helper", &mut inputs))
            .and_then(|helper| self.child(helper, "Helper", &mut inputs));
        if let Some((attachment, helper)) = attachment
            .zip(helper)
            .filter(|_| definition.transform().is_some())
        {
            let mut kind = inputs.clone();
            let text = RecordJsonLocation::borrowed_text(self.forge, attachment.type_name());
            kind.push(Input {
                role: SourceUseRole::Conversion,
                complete: text.is_some(),
                text,
                ..Input::structure(attachment)
            });
            self.push("/transform/attachment", kind);
            for (wire, source) in [
                ("helper", "Name"),
                ("offset_helper", "ItemOffsetHelperName"),
            ] {
                let mut field = inputs.clone();
                field.extend(self.field(helper, source, SourceUseRole::Payload));
                self.push(format!("/transform/{wire}"), field);
            }
            self.transform_axes(helper, &inputs);
        }
        self.push("/transform", inputs);
    }

    fn transform_axes(&mut self, helper: &StructValue<'_>, inputs: &[Input]) {
        let mut shared = inputs.to_vec();
        let offset = self.child(helper, "Offset", &mut shared);
        for (wire, source) in [("position", "Position"), ("rotation", "Rotation")] {
            let mut route = shared.clone();
            let vector = offset.and_then(|offset| self.child(offset, source, &mut route));
            let path = format!("/transform/{wire}");
            let mut aggregate = route.clone();
            for (index, axis) in ["x", "y", "z"].into_iter().enumerate() {
                let mut axis_inputs = route.clone();
                if let Some(vector) = vector {
                    let input = self.field(vector, axis, SourceUseRole::Conversion);
                    aggregate.extend(input.iter().cloned());
                    axis_inputs.extend(input);
                }
                self.push(format!("{path}/{index}"), axis_inputs);
            }
            self.push(path, aggregate);
        }
    }

    fn child<'tree, 'source>(
        &self,
        parent: &'tree StructValue<'source>,
        field: &str,
        inputs: &mut Vec<Input>,
    ) -> Option<&'tree StructValue<'source>> {
        inputs.push(Input::structure(parent));
        let value = parent.field(field)?;
        inputs.push(Input::value(self.forge, value, SourceUseRole::Selector));
        match value {
            Value::Struct(child) => Some(child),
            _ => None,
        }
    }
}

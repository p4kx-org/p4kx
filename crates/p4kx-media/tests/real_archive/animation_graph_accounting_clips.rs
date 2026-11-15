// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::cryxml::{
    SourceContent, SourceContents, SourceDeclaration, SourceDeclarationKind, SourceDocument,
    SourceElement,
};
use p4kx_media::MediaRecord;
use serde_json::{Map, Value, json};

use super::source::attribute_position;
use super::{TestResult, require};

pub(crate) const NAMESPACE: &str = "animation-definition-clip-reference";

#[derive(Clone, Copy)]
pub(crate) enum Format {
    Mannequin,
    BlendSpace,
}

#[derive(Clone, Copy)]
pub(crate) enum Scope {
    Standalone,
    Scoped,
}

#[derive(Default)]
pub(crate) struct Clips(BTreeMap<[u64; 2], Value>);

#[derive(Clone, Copy)]
struct Element<'a> {
    name: Option<&'a str>,
    ordinal: u64,
}

struct Frame<'a> {
    element: Element<'a>,
    content: SourceContents<'a>,
}

enum Owner {
    Fragment {
        fragment: u64,
        parent: u64,
        parent_kind: &'static str,
        layer: u64,
    },
    Example(u64),
}

impl Clips {
    pub(crate) fn read(
        document: &SourceDocument<'_>,
        format: Format,
        scope: Scope,
    ) -> TestResult<Self> {
        let mut result = Self::default();
        if document.is_lossy() {
            return Ok(result);
        }
        let mut next = 0;
        let mut frames = vec![Frame {
            element: Element {
                name: None,
                ordinal: 0,
            },
            content: document.content(),
        }];
        while let Some(frame) = frames.last_mut() {
            let Some(item) = frame.content.next() else {
                frames.pop();
                continue;
            };
            let index = next;
            next += 1;
            if let SourceContent::Element(element) = item? {
                let site = located(element, index);
                let ancestry = frames.get(1..).ok_or("original XML document frame")?;
                if let Some(owner) = owner(ancestry, format, site) {
                    result.add(element, index, &owner, scope)?;
                }
                frames.push(Frame {
                    element: site,
                    content: element.syntax()?.content()?,
                });
            }
        }
        Ok(result)
    }

    fn add(
        &mut self,
        element: SourceElement<'_>,
        ordinal: u64,
        owner: &Owner,
        scope: Scope,
    ) -> TestResult {
        for (index, declaration) in element.syntax()?.declarations().enumerate() {
            let declaration = declaration?;
            if matches!(
                declaration.kind(),
                SourceDeclarationKind::Attribute { namespace: None }
            ) && declaration.qualified_name() == owner.selector()
            {
                self.0
                    .insert([ordinal, index as u64], owner.payload(&declaration, scope)?);
            }
        }
        Ok(())
    }

    pub(crate) fn observe(&mut self, row: &MediaRecord) -> TestResult {
        let [element, declaration] = row.subresource.indices.as_slice() else {
            return Err("ClipReference identity must include both original ordinals".into());
        };
        let expected = self
            .0
            .remove(&[*element, *declaration])
            .ok_or("unexpected or duplicate original clip selector")?;
        require(
            row.data.get("record_type").and_then(Value::as_str) == Some("clip-reference")
                && row.data.get("payload") == Some(&expected),
            "clip declaration preserves exact selector, ancestry, position and resolution",
        )
    }

    pub(crate) fn remaining(&self) -> usize {
        self.0.len()
    }
}

fn located(element: SourceElement<'_>, ordinal: u64) -> Element<'_> {
    Element {
        name: element.namespace().is_none().then(|| element.name()),
        ordinal,
    }
}

fn owner(path: &[Frame<'_>], format: Format, leaf: Element<'_>) -> Option<Owner> {
    let is = |frame: &Frame<'_>, name| frame.element.name == Some(name);
    match (format, path, leaf.name) {
        (Format::Mannequin, [root, list, group, fragment, layer], Some("Animation"))
            if is(root, "AnimDB")
                && is(list, "FragmentList")
                && group.element.name.is_some()
                && is(fragment, "Fragment")
                && is(layer, "AnimLayer") =>
        {
            Some(fragment_owner(group, "fragment-group", fragment, layer))
        }
        (Format::Mannequin, [root, list, blend, option, fragment, layer], Some("Animation"))
            if is(root, "AnimDB")
                && is(list, "FragmentBlendList")
                && is(blend, "Blend")
                && option.element.name.is_some()
                && is(fragment, "Fragment")
                && is(layer, "AnimLayer") =>
        {
            Some(fragment_owner(option, "transition-option", fragment, layer))
        }
        (Format::BlendSpace, [root, list], Some("Example"))
            if is(root, "ParaGroup") && is(list, "ExampleList") =>
        {
            Some(Owner::Example(leaf.ordinal))
        }
        _ => None,
    }
}

const fn fragment_owner(
    parent: &Frame<'_>,
    parent_kind: &'static str,
    fragment: &Frame<'_>,
    layer: &Frame<'_>,
) -> Owner {
    Owner::Fragment {
        fragment: fragment.element.ordinal,
        parent: parent.element.ordinal,
        parent_kind,
        layer: layer.element.ordinal,
    }
}

impl Owner {
    const fn selector(&self) -> &'static str {
        match self {
            Self::Fragment { .. } => "name",
            Self::Example(_) => "AName",
        }
    }

    fn payload(&self, declaration: &SourceDeclaration<'_>, scope: Scope) -> TestResult<Value> {
        let name = declaration.value();
        let reason = match (name.is_empty(), self, scope) {
            (true, _, _) => "empty-selector",
            (_, _, Scope::Scoped) => "placement-scoped",
            _ => "no-owning-scope",
        };
        let mut payload = Map::from_iter([
            (
                "selector_position".to_owned(),
                attribute_position(declaration.position())?,
            ),
            ("authored_name".to_owned(), json!(name)),
            (
                "resolution".to_owned(),
                json!({"state": "unresolved", "reason": reason}),
            ),
        ]);
        match self {
            Self::Fragment {
                fragment,
                parent,
                parent_kind,
                layer,
            } => {
                payload.insert(
                    "owner".to_owned(),
                    coordinate("mannequin-fragment", *fragment),
                );
                payload.insert("owner_parent".to_owned(), coordinate(parent_kind, *parent));
                payload.insert("layer_ordinal".to_owned(), json!(layer.to_string()));
            }
            Self::Example(index) => {
                payload.insert(
                    "owner".to_owned(),
                    coordinate("blend-space-example", *index),
                );
            }
        }
        Ok(Value::Object(payload))
    }
}

fn coordinate(kind: &str, ordinal: u64) -> Value {
    json!({"kind": kind, "element_ordinal": ordinal.to_string()})
}

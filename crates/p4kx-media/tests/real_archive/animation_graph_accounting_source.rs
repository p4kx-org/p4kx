// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceContent, SourceContents, SourceDeclaration,
    SourceDeclarationKind, SourceDocument, SourceElement, SourceElementPosition,
};
use p4kx_media::{ContainerIdentity, MediaRecord, Status};
use serde_json::{Value, json};

use super::links::{self, Edges, Paths};
use super::{Family, TestResult, require};

#[derive(Clone, Copy)]
struct Site {
    index: u64,
    parent: Option<u64>,
    sibling: u64,
}

struct Frame<'a> {
    parent: Option<u64>,
    content: SourceContents<'a>,
    next_sibling: u64,
}

struct Node<'a> {
    element: SourceElement<'a>,
    site: Site,
}

pub(super) struct ExpectedSource<'a> {
    raw_nodes: BTreeMap<u64, Node<'a>>,
    raw_attributes: BTreeMap<[u64; 2], SourceDeclaration<'a>>,
    graph_roots: BTreeMap<u64, Node<'a>>,
    link_declarations: BTreeMap<[u64; 2], links::Selector<'a>>,
    sub_lists: BTreeSet<u64>,
    nodes: u64,
    family: Family,
    known_root: bool,
}

impl<'a> ExpectedSource<'a> {
    pub(super) fn new(document: &'a SourceDocument<'a>, family: Family) -> TestResult<Self> {
        let mut expected = Self {
            raw_nodes: BTreeMap::new(),
            raw_attributes: BTreeMap::new(),
            graph_roots: BTreeMap::new(),
            link_declarations: BTreeMap::new(),
            sub_lists: BTreeSet::new(),
            nodes: 0,
            family,
            known_root: false,
        };
        let mut stack = vec![Frame {
            parent: None,
            content: document.content(),
            next_sibling: 0,
        }];
        while let Some(frame) = stack.last_mut() {
            let Some(item) = frame.content.next() else {
                stack.pop();
                continue;
            };
            let site = Site {
                index: expected.nodes,
                parent: frame.parent,
                sibling: frame.next_sibling,
            };
            expected.nodes += 1;
            frame.next_sibling += 1;
            if let SourceContent::Element(element) = item? {
                expected.observe(element, site)?;
                stack.push(Frame {
                    parent: Some(site.index),
                    content: element.syntax()?.content()?,
                    next_sibling: 0,
                });
            }
        }
        Ok(expected)
    }

    fn observe(&mut self, element: SourceElement<'a>, site: Site) -> TestResult {
        let syntax = element.syntax()?;
        if site.parent.is_none()
            && element.namespace().is_none()
            && element.name() == self.family.root()
        {
            self.known_root = true;
            self.graph_roots.insert(site.index, Node { element, site });
        }
        if self.family == Family::Adb
            && element.namespace().is_none()
            && element.name() == "SubADBs"
            && site
                .parent
                .is_some_and(|parent| self.graph_roots.contains_key(&parent))
        {
            self.sub_lists.insert(site.index);
        }
        let edge_allowed = site
            .parent
            .is_some_and(|parent| self.sub_lists.contains(&parent))
            && element.name() == "SubADB"
            && element
                .attributes()
                .filter(|attribute| {
                    attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case("File")
                })
                .count()
                == 1;
        for (ordinal, declaration) in syntax.declarations().enumerate() {
            let declaration = declaration?;
            let coordinate = [site.index, ordinal as u64];
            if let Some(selector) = links::Selector::of(
                self.family,
                element,
                site.parent.is_none(),
                &declaration,
                edge_allowed,
            ) {
                self.link_declarations.insert(coordinate, selector);
            }
            self.raw_attributes.insert(coordinate, declaration);
        }
        self.raw_nodes.insert(site.index, Node { element, site });
        Ok(())
    }

    pub(super) const fn known_root(&self) -> bool {
        self.known_root
    }

    pub(super) const fn node_count(&self) -> u64 {
        self.nodes
    }

    pub(super) fn verify(
        &mut self,
        row: &MediaRecord,
        paths: &Paths<'_>,
        chain: &[ContainerIdentity],
        entry: usize,
        edges: &mut Edges,
    ) -> TestResult {
        require(
            row.status == Status::Complete,
            "a graph row has a diagnosed loss",
        )?;
        let indices = row.subresource.indices.as_slice();
        let namespace = row.subresource.namespace.as_str();
        let payload = row.data.get("payload").ok_or("graph payload missing")?;
        match (namespace, indices) {
            ("animation-definition-element", [index]) => {
                let expected = self
                    .raw_nodes
                    .remove(index)
                    .ok_or("unexpected source element row")?;
                expected.verify(payload, "qualified_name")
            }
            ("animation-definition-declaration", [index, ordinal]) => {
                let expected = self
                    .raw_attributes
                    .remove(&[*index, *ordinal])
                    .ok_or("unexpected source declaration row")?;
                declaration(payload, &expected)
            }
            ("animation-definition-graph-node", [index]) => {
                if let Some(expected) = self.graph_roots.remove(index) {
                    require(
                        payload.get("role").and_then(Value::as_str)
                            == Some(self.family.root_role()),
                        "typed root matches original document family",
                    )?;
                    expected.verify(payload, "name")?;
                }
                Ok(())
            }
            ("animation-definition-link", [index, ordinal]) => {
                let coordinate = [*index, *ordinal];
                let selector = self
                    .link_declarations
                    .remove(&coordinate)
                    .ok_or("unexpected scoped link row")?;
                if let Some(target) = selector.verify(payload, paths, chain)?
                    && selector.edge_allowed()
                {
                    edges.observe(entry, coordinate, target);
                }
                Ok(())
            }
            ("animation-definition-named-caf" | "animation-definition-named-dba-binding", _) => {
                Err("standalone authored graph cannot emit a CAF binding row".into())
            }
            _ => Ok(()),
        }
    }

    pub(super) fn finish(&self) -> TestResult {
        require(
            self.raw_nodes.is_empty()
                && self.raw_attributes.is_empty()
                && self.graph_roots.is_empty()
                && self.link_declarations.is_empty(),
            "original nodes, declarations, typed root and file links are completely represented",
        )
    }
}

impl Node<'_> {
    fn verify(&self, payload: &Value, name: &str) -> TestResult {
        let element = &self.element;
        let syntax = element.syntax()?;
        let expected = element_position(&element.position())?;
        require(
            payload.get(name).and_then(Value::as_str) == Some(syntax.qualified_name()),
            "original element qualified name",
        )?;
        require(
            payload.get("position") == Some(&expected),
            "original element coordinate",
        )?;
        let parent = self.site.parent.map(|value| value.to_string());
        let expected = json!({"parent": parent, "sibling_ordinal": self.site.sibling.to_string()});
        require(
            payload.get("location") == Some(&expected),
            "same original parent and sibling order",
        )
    }
}

fn declaration(payload: &Value, original: &SourceDeclaration<'_>) -> TestResult {
    let position = attribute_position(original.position())?;
    let kind = match original.kind() {
        SourceDeclarationKind::Attribute { namespace } => {
            json!({"type":"attribute","namespace":namespace})
        }
        SourceDeclarationKind::Namespace { prefix } => json!({"type":"namespace","prefix":prefix}),
        _ => return Err("unrecognized original graph declaration kind".into()),
    };
    let expected = json!({
        "qualified_name": original.qualified_name(),
        "value": original.value(),
        "lexeme": original.lexeme(),
        "kind": kind,
        "position": position,
    });
    require(
        payload == &expected,
        "original declaration value, lexical form and coordinates",
    )
}

fn element_position(position: &SourceElementPosition) -> TestResult<Value> {
    match position {
        SourceElementPosition::Text(range) => Ok(text(range)),
        SourceElementPosition::CryXml(node_index) => {
            Ok(json!({"type":"cry-xml-node","node_index":node_index}))
        }
        _ => Err("unsupported source element coordinate".into()),
    }
}

fn attribute_position(position: &SourceAttributePosition) -> TestResult<Value> {
    match position {
        SourceAttributePosition::Text(range) => Ok(text(range)),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
            ..
        } => Ok(
            json!({"type":"cry-xml-attribute","node_index":node_index,"attribute_index":attribute_index}),
        ),
        _ => Err("unsupported source declaration coordinate".into()),
    }
}

fn text(range: &Range<usize>) -> Value {
    json!({"type":"text","start":range.start.to_string(),"end_exclusive":range.end.to_string()})
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CigAudioRole {
    Graph,
    Objects,
    Object,
    Events,
    EventReference,
    Parameters,
    ParameterReference,
    ParameterHashes,
    Switches,
    Logic,
    Nodes,
    LogicNode,
    InheritedNodes,
    Links,
    Link,
    InputLinks,
    InputLink,
    OutputLinks,
    OutputLink,
    EditorPosition,
}

impl CigAudioRole {
    pub(crate) fn root(element: SourceElement<'_>) -> Option<Self> {
        (element.name() == "CIGAudioData" && element.namespace().is_none()).then_some(Self::Graph)
    }

    pub(crate) fn child(self, element: SourceElement<'_>) -> Option<Self> {
        if element.namespace().is_some() {
            return None;
        }
        let name = element.name();
        Some(match (self, name) {
            (Self::Graph, "objects") => Self::Objects,
            (Self::Objects, name) if list_entry(name) => Self::Object,
            (Self::Object, "events") => Self::Events,
            (Self::Events, name) if list_entry(name) => Self::EventReference,
            (Self::Object, "parameters") => Self::Parameters,
            (Self::Parameters, name) if list_entry(name) => Self::ParameterReference,
            (Self::Object, "parameterhashes") => Self::ParameterHashes,
            (Self::Object, "switches") => Self::Switches,
            (Self::Object, "logic") => Self::Logic,
            (Self::Logic, "nodes") => Self::Nodes,
            (Self::Nodes, name) if list_entry(name) => Self::LogicNode,
            (Self::Logic, "inheritedNodes") => Self::InheritedNodes,
            (Self::InputLink | Self::OutputLink, "links") => Self::Links,
            (Self::Links, name) if list_entry(name) => Self::Link,
            (Self::LogicNode, "inLinks") => Self::InputLinks,
            (Self::InputLinks, name) if list_entry(name) => Self::InputLink,
            (Self::LogicNode, "outLinks") => Self::OutputLinks,
            (Self::OutputLinks, name) if list_entry(name) => Self::OutputLink,
            (Self::LogicNode, "editorPosition") => Self::EditorPosition,
            _ => return None,
        })
    }
}

fn list_entry(name: &str) -> bool {
    !name.is_empty()
        && name
            .strip_prefix('_')
            .unwrap_or(name)
            .bytes()
            .all(|byte| byte.is_ascii_digit())
}

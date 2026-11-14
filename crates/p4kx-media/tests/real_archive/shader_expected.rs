// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source-derived expectations for a bounded shader export.

use std::collections::BTreeMap;

use p4kx_media::Identity;
use p4kx_shader::Declaration;

use super::TestResult;

pub(super) struct ExpectedSource {
    pub(super) identity: Identity,
    pub(super) size: u64,
    pub(super) digest: String,
    pub(super) counts: Option<ExpectedCounts>,
    pub(super) failure: Option<p4kx_shader::SyntaxError>,
}

pub(super) struct ExpectedCounts {
    pub(super) nodes: BTreeMap<Vec<u64>, ExpectedNode>,
    pub(super) root_declarations: usize,
    pub(super) properties: Vec<ExpectedProperty>,
}

pub(super) struct ExpectedNode {
    pub(super) span: p4kx_shader::Span,
    pub(super) children: usize,
    pub(super) property: bool,
}

pub(super) struct ExpectedProperty {
    pub(super) ordinal: usize,
    pub(super) root_index: usize,
    pub(super) span: p4kx_shader::Span,
}

impl ExpectedCounts {
    pub(super) fn of(definition: &p4kx_shader::Definition<'_>) -> TestResult<Self> {
        let mut properties = Vec::new();
        for (index, field) in definition.fields().enumerate() {
            if field.declaration().form == p4kx_shader::Form::Block
                && field
                    .name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("Property"))
            {
                properties.push(ExpectedProperty {
                    ordinal: properties.len(),
                    root_index: index,
                    span: field.declaration().span,
                });
            }
        }
        let mut nodes = BTreeMap::new();
        collect(definition.declarations(), &mut Vec::new(), &mut nodes)?;
        for property in &properties {
            let key = [u64::try_from(property.root_index)?];
            nodes
                .get_mut(key.as_slice())
                .ok_or("source Property node missing")?
                .property = true;
        }
        Ok(Self {
            nodes,
            root_declarations: definition.declarations().len(),
            properties,
        })
    }
}

fn collect(
    declarations: &[Declaration],
    path: &mut Vec<u64>,
    nodes: &mut BTreeMap<Vec<u64>, ExpectedNode>,
) -> TestResult {
    for (index, declaration) in declarations.iter().enumerate() {
        path.push(u64::try_from(index)?);
        let previous = nodes.insert(
            path.clone(),
            ExpectedNode {
                span: declaration.span,
                children: declaration.children.len(),
                property: false,
            },
        );
        if previous.is_some() {
            return Err("duplicate source declaration path".into());
        }
        collect(&declaration.children, path, nodes)?;
        let _ordinal = path.pop();
    }
    Ok(())
}

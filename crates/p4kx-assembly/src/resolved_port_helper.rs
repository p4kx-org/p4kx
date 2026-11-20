// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::{self, Display, Formatter};

use crate::port_sub_helper::{AuthoredHelperTags, HelperChildren};
use crate::{
    Attachment, PortDefinition, PortDefinitionPosition, PortHelperChoice, PortHelperProblem,
    PortHelperTagMatch, PortNode, PortTransform,
};

/// The physical port and helper occurrence whose placement selector could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortHelperDiagnostic {
    port: PortDefinitionPosition,
    attached_record_index: Option<u32>,
    helper_ordinals: Box<[usize]>,
    problem: PortHelperProblem,
}

impl PortHelperDiagnostic {
    /// Returns the original port declaration.
    #[must_use]
    pub const fn port(&self) -> PortDefinitionPosition {
        self.port
    }

    /// Returns the physical Record of the resolved attachment.
    #[must_use]
    pub const fn attached_record_index(&self) -> Option<u32> {
        self.attached_record_index
    }

    /// Borrows the original child ordinals from the root helper to this occurrence.
    #[must_use]
    pub const fn helper_ordinals(&self) -> &[usize] {
        &self.helper_ordinals
    }

    /// Returns the reason selection was unresolved.
    #[must_use]
    pub const fn problem(&self) -> PortHelperProblem {
        self.problem
    }
}

impl Display for PortHelperDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let port = self.port;
        write!(
            formatter,
            "unresolved port helper at Record {} Components[{}].Ports[{}].AttachmentImplementation.Helper",
            port.record_index(),
            port.component_ordinal(),
            port.port_ordinal(),
        )?;
        for ordinal in &self.helper_ordinals {
            write!(formatter, ".SubHelpers[{ordinal}]")?;
        }
        if let Some(record) = self.attached_record_index {
            write!(formatter, " for attached Record {record}")?;
        }
        write!(formatter, ": {}", reason(self.problem))
    }
}

impl std::error::Error for PortHelperDiagnostic {}

const fn reason(problem: PortHelperProblem) -> &'static str {
    match problem {
        PortHelperProblem::UnreadableTree => "the helper tree is unreadable",
        PortHelperProblem::TaggedRoot => "the root helper has an unproven selector",
        PortHelperProblem::UnreadableSelector => "the selector is unreadable",
        PortHelperProblem::UnsupportedSelector => "the selector has no proven evaluation rule",
        PortHelperProblem::UnreadableChildren => "the child array is unreadable",
        PortHelperProblem::UnreadableChild(_) => "the child is unreadable",
        PortHelperProblem::UnusableHelper => "the declared helper is unusable",
    }
}

#[derive(Default)]
pub(crate) struct ResolvedPortHelper<'port> {
    pub(crate) transform: Option<&'port PortTransform>,
    pub(crate) diagnostic: Option<PortHelperDiagnostic>,
}

impl PortNode {
    /// Returns the placement used for this attachment and any unresolved helper evidence.
    pub(crate) fn placement_helper(&self) -> ResolvedPortHelper<'_> {
        let Some(definition) = self.definition() else {
            return ResolvedPortHelper::default();
        };
        match self.attachment() {
            Attachment::Entity(item) => resolve(definition, item.record_index()),
            _ => ResolvedPortHelper {
                transform: definition.transform(),
                diagnostic: None,
            },
        }
    }
}

pub(crate) fn resolve(
    definition: &PortDefinition,
    attached_record_index: Option<u32>,
) -> ResolvedPortHelper<'_> {
    ResolvedPortHelper {
        transform: definition.transform(),
        diagnostic: (!definition.is_skin())
            .then(|| unresolved_helper(definition, attached_record_index))
            .flatten(),
    }
}

fn unresolved_helper(
    definition: &PortDefinition,
    attached_record_index: Option<u32>,
) -> Option<PortHelperDiagnostic> {
    let tree = &definition.helper_tree;
    let ordinary_root = match &tree.root_tags {
        AuthoredHelperTags::Absent => true,
        AuthoredHelperTags::Text(tags) => tags.trim().is_empty(),
        AuthoredHelperTags::Unreadable => false,
    };
    if ordinary_root
        && !tree.root_unreadable
        && tree.root_children == HelperChildren::Read
        && tree.nodes.is_empty()
    {
        return None;
    }

    let selection = definition.select_helper(|_| PortHelperTagMatch::Unsupported);
    let PortHelperChoice::Unresolved { path, problem } = selection.choice else {
        return None;
    };
    let mut helper_ordinals = path.ordinals();
    if let PortHelperProblem::UnreadableChild(ordinal) = problem {
        helper_ordinals.push(ordinal);
    }
    Some(PortHelperDiagnostic {
        port: definition.position(),
        attached_record_index,
        helper_ordinals: helper_ordinals.into_boxed_slice(),
        problem,
    })
}

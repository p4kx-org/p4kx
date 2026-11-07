// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityNode, LoadoutError, PortNode, Target};
use p4kx_extraction::{Guid, Record, ValueError};

use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode as Code, ExportDiagnosticSeverity as Severity,
    ExportDiagnosticStage, ExportSourceRole,
};
use crate::export_ledger::ExportAccountingError;

type Errors<'n> =
    std::iter::Chain<std::slice::Iter<'n, LoadoutError>, std::slice::Iter<'n, LoadoutError>>;

enum Frame<'n> {
    Node(&'n EntityNode),
    Ports(std::slice::Iter<'n, PortNode>),
    Errors(Errors<'n>),
}

struct Issue<'n> {
    error: &'n LoadoutError,
    target: Option<Guid>,
}

struct Issues<'n> {
    stack: Vec<Frame<'n>>,
}

impl<'n> Iterator for Issues<'n> {
    type Item = Issue<'n>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(frame) = self.stack.pop() {
            match frame {
                Frame::Node(node) => {
                    self.stack.push(Frame::Ports(node.ports().iter()));
                    self.stack.push(Frame::Errors(
                        node.issues().iter().chain(node.notes().iter()),
                    ));
                }
                Frame::Errors(mut errors) => {
                    if let Some(error) = errors.next() {
                        self.stack.push(Frame::Errors(errors));
                        return Some(Issue {
                            error,
                            target: target(error),
                        });
                    }
                }
                Frame::Ports(mut ports) => {
                    let Some(port) = ports.next() else {
                        continue;
                    };
                    self.stack.push(Frame::Ports(ports));
                    match port.attachment() {
                        Attachment::Empty => self.stack.push(Frame::Ports(port.nested().iter())),
                        Attachment::Entity(child) if child.is_expanded() => {
                            self.stack.push(Frame::Node(child));
                        }
                        Attachment::Entity(_) => {}
                        Attachment::Unresolved(unresolved) => {
                            let target = match unresolved.target() {
                                Target::Reference(guid) => Some(*guid),
                                _ => target(unresolved.error()),
                            };
                            return Some(Issue {
                                error: unresolved.error(),
                                target,
                            });
                        }
                    }
                }
            }
        }
        None
    }
}

const fn target(error: &LoadoutError) -> Option<Guid> {
    match error {
        LoadoutError::DanglingReference { guid }
        | LoadoutError::Cycle { guid }
        | LoadoutError::UnreadableRecord { guid, .. }
        | LoadoutError::NotAnEntity { guid, .. }
        | LoadoutError::Assortment { guid, .. } => Some(*guid),
        LoadoutError::InvalidReference { reason } => reason.dangling_guid(),
        _ => None,
    }
}

const fn category(error: &LoadoutError) -> (Code, Severity) {
    match error {
        LoadoutError::MissingXmlLoadout { .. }
        | LoadoutError::MissingVehicleImplementation { .. }
        | LoadoutError::DanglingReference { .. }
        | LoadoutError::InvalidReference {
            reason: ValueError::DanglingReference { .. },
        } => (Code::DanglingReference, Severity::Information),
        LoadoutError::InvalidReference {
            reason: ValueError::OutsideRecord { .. },
        } => (Code::OutsideOwner, Severity::Information),
        LoadoutError::UnknownClass { .. }
        | LoadoutError::AmbiguousClass { .. }
        | LoadoutError::Cycle { .. }
        | LoadoutError::UnknownPick { .. }
        | LoadoutError::AssortmentRequirement { .. } => {
            (Code::SourceWarning, Severity::Information)
        }
        LoadoutError::TooDeep | LoadoutError::TooManyPorts => {
            (Code::FieldWalkLimit, Severity::Warning)
        }
        LoadoutError::UnreadableRecord { .. } | LoadoutError::InvalidReference { .. } => {
            (Code::UnreadableSource, Severity::Error)
        }
        LoadoutError::NoLoadoutFiles { .. } | LoadoutError::XmlLoadout { .. } => {
            (Code::SourceWarning, Severity::Warning)
        }
        _ => (Code::UnsupportedValue, Severity::Warning),
    }
}

pub(super) fn diagnostics(
    record: &Record<'_>,
    node: &EntityNode,
) -> Result<Vec<ExportDiagnostic>, ExportAccountingError> {
    let issues = Issues {
        stack: vec![Frame::Node(node)],
    };
    crate::export_health_compile::aggregate(issues.map(|issue| {
        let (code, severity) = category(issue.error);
        let mut diagnostic = super::operation::diagnostic(
            record,
            ExportDiagnosticStage::DerivedProjection,
            code,
            severity,
            issue.error.kind(),
        );
        diagnostic.source_role = Some(ExportSourceRole::DerivedInput);
        if let Some(example) = diagnostic.examples.first_mut() {
            example.target_guid = issue.target.map(|guid| guid.to_string());
            if let LoadoutError::MissingXmlLoadout { path }
            | LoadoutError::MissingVehicleImplementation { path } = issue.error
            {
                example.field = Some(path.clone());
            }
        }
        diagnostic
    }))
}

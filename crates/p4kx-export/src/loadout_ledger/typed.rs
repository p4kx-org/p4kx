// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutClassSelection, LoadoutError, LoadoutReferenceSelection,
    LoadoutXmlClassUse, PortNode, Target,
};
use p4kx_extraction::{Guid, ValueError};

use crate::export_diagnostic::{
    ExportDiagnosticCode as Code, ExportDiagnosticSeverity as Severity, ExportDiagnosticStage,
    ExportSourceRole,
};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

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
    reference: Option<&'n LoadoutReferenceSelection>,
    class: Option<(&'n LoadoutClassSelection, Option<&'n LoadoutXmlClassUse>)>,
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
                            reference: reference(error),
                            class: class(error),
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
                                reference: unresolved
                                    .reference_selection()
                                    .or_else(|| reference(unresolved.error())),
                                class: class(unresolved.error()),
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
        | LoadoutError::Assortment { guid, .. } => Some(guid.guid()),
        LoadoutError::InvalidReference { reason } => reason.dangling_guid(),
        _ => None,
    }
}

const fn reference(error: &LoadoutError) -> Option<&LoadoutReferenceSelection> {
    match error {
        LoadoutError::DanglingReference { guid }
        | LoadoutError::Cycle { guid }
        | LoadoutError::UnreadableRecord { guid, .. }
        | LoadoutError::NotAnEntity { guid, .. }
        | LoadoutError::Assortment { guid, .. } => guid.selection(),
        _ => None,
    }
}

fn class(error: &LoadoutError) -> Option<(&LoadoutClassSelection, Option<&LoadoutXmlClassUse>)> {
    match error {
        LoadoutError::DanglingReference { guid }
        | LoadoutError::Cycle { guid }
        | LoadoutError::UnreadableRecord { guid, .. }
        | LoadoutError::NotAnEntity { guid, .. }
        | LoadoutError::Assortment { guid, .. } => guid
            .class_selection()
            .map(|selection| (selection, guid.class_xml_use())),
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
    capture: &super::Capture<'_>,
    admission: &RecordObservation<'_, '_>,
    node: &EntityNode,
) -> Result<Vec<ExportDiagnostic>, CatalogError> {
    let issues = Issues {
        stack: vec![Frame::Node(node)],
    };
    crate::export_health_aggregate::aggregate_fallible(issues.map(|issue| {
        let (code, severity) = category(issue.error);
        let example = if let (Some(_), Some(guid)) = (capture.input, issue.target) {
            match (issue.reference, issue.class) {
                (Some(reference), None) => capture.reference_example(admission, reference, guid)?,
                (None, Some((class, xml_use))) => {
                    capture.class_example(admission, class, guid, xml_use)?
                }
                (None, None) => return Err(ExportAccountingError::UnboundHealthSubject.into()),
                (Some(_), Some(_)) => {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            }
        } else {
            let mut example = match issue.error {
                LoadoutError::MissingXmlLoadout { path }
                | LoadoutError::MissingVehicleImplementation { path } => {
                    capture.request_example(admission, path)?
                }
                _ => super::operation::example(capture.input, admission)?,
            };
            example.target_guid = issue.target;
            example
        };
        let mut diagnostic = super::operation::from_example(
            ExportDiagnosticStage::DerivedProjection,
            code,
            severity,
            issue.error.kind(),
            example,
        );
        diagnostic.source_role = Some(ExportSourceRole::DerivedInput);
        Ok::<_, CatalogError>(diagnostic)
    }))
}

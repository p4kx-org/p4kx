// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashSet};

use p4kx_dataforge::{DataForge, RecordTree, Reference, Step, StructValue, Value};

use super::{Check, Trace, original};
use crate::bindings;
use crate::reader;
use crate::report::{ContentOmissionCode, OmissionInput, Report};
use crate::{Variable, WidgetInput};

#[derive(Debug, Default)]
struct InputChain {
    operations: BTreeSet<usize>,
    edges: BTreeSet<LocalInput>,
    supplied_variables: BTreeSet<usize>,
    absent_variables: BTreeSet<usize>,
    unverified_edges: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct InputSite {
    operation: usize,
    value_ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LocalInput {
    from: InputSite,
    target: usize,
}

pub(super) struct BindingInputs<'read, 'source> {
    forge: &'read DataForge<'source>,
    variables: &'read [Variable],
    report: &'read Report,
}

impl<'read, 'source> BindingInputs<'read, 'source> {
    pub(super) const fn new(
        forge: &'read DataForge<'source>,
        variables: &'read [Variable],
        report: &'read Report,
    ) -> Self {
        Self {
            forge,
            variables,
            report,
        }
    }
}

#[derive(Debug)]
struct BindingSite {
    target: WidgetInput,
    operation: usize,
    field: Option<String>,
    direct_target: bool,
    input: InputChain,
    omissions: Vec<ContentOmissionCode>,
}

#[derive(Default)]
pub(super) struct BindingEvidence {
    seen: HashSet<WidgetInput>,
    sites: Vec<BindingSite>,
}

#[derive(Default)]
pub(super) struct BindingCounts {
    pub(super) sites: usize,
    pub(super) graph_nodes: usize,
    pub(super) local_edges: usize,
    pub(super) supplied: usize,
    pub(super) absent: usize,
    pub(super) source_gaps: usize,
    pub(super) reported_omissions: usize,
    pub(super) background_targets: usize,
}

impl BindingEvidence {
    pub(super) fn inspect_ancestors(
        &mut self,
        trace: &Trace,
        index: usize,
        inputs: &BindingInputs<'_, '_>,
    ) -> Check {
        let mut current = Some(index);
        while let Some(visit) = current.and_then(|at| trace.visits.get(at)) {
            self.inspect(inputs, visit.input)?;
            current = visit.parent;
        }
        Ok(())
    }

    fn inspect(&mut self, inputs: &BindingInputs<'_, '_>, widget: WidgetInput) -> Check {
        if !self.seen.insert(widget) {
            return Ok(());
        }
        let owner = original::tree(inputs.forge, widget.record_index)?;
        let operations = reader::items(owner.root(), "operations");
        let path = owner.record_path();
        let canvas = path.rsplit_once('/').map_or(path, |(_, name)| name);
        for (operation, value) in operations.iter().enumerate() {
            let Some(source) = reader::as_struct(value) else {
                continue;
            };
            let direct_target = reader::reference(source, "widget")
                .is_some_and(|reference| is_widget(reference, &owner, widget));
            if !direct_target && !alternative_target(source, &owner, widget) {
                continue;
            }

            let omissions = inputs
                .report
                .content_omissions
                .iter()
                .filter(|entry| {
                    entry.widget_input == widget
                        && matches!(entry.input, OmissionInput::Binding { operation: at, .. }
                            if operation == at)
                })
                .map(|entry| entry.code)
                .collect();
            self.sites.push(BindingSite {
                target: widget,
                operation,
                field: reader::text(source, "field").map(str::to_owned),
                direct_target,
                input: trace_inputs(&owner, operations, operation, canvas, inputs.variables),
                omissions,
            });
        }
        Ok(())
    }

    pub(super) fn counts(&self) -> BindingCounts {
        let mut counts = BindingCounts {
            sites: self.sites().count(),
            ..BindingCounts::default()
        };
        for site in &self.sites {
            counts.graph_nodes += site.input.operations.len().saturating_sub(1);
            counts.local_edges += site.input.edges.len();
            counts.source_gaps += site.input.unverified_edges;
            counts.source_gaps += usize::from(!site.direct_target || site.field.is_none());
            counts.reported_omissions += site.omissions.len();
            counts.background_targets += usize::from(matches!(
                site.field.as_deref(),
                Some("EnableBackground" | "BackgroundColor")
            ));
            counts.supplied += site.input.supplied_variables.len();
            counts.absent += site.input.absent_variables.len();
        }
        counts
    }

    fn sites(&self) -> impl Iterator<Item = (WidgetInput, usize)> + '_ {
        self.sites.iter().map(|site| (site.target, site.operation))
    }
}

fn is_widget(reference: &Reference<'_>, owner: &RecordTree<'_>, widget: WidgetInput) -> bool {
    reference.record_index() == owner.record_index()
        && original::same_widget(reference, owner.guid(), widget)
}

fn alternative_target(
    source: &StructValue<'_>,
    owner: &RecordTree<'_>,
    widget: WidgetInput,
) -> bool {
    let mut found = false;
    source.visit_owned_values(|visit| {
        if visit.steps() != [Step::Field("widget")]
            && let Value::Reference(reference) = visit.value()
        {
            found |= is_widget(reference, owner, widget);
        }
    });
    found
}

fn trace_inputs(
    owner: &RecordTree<'_>,
    operations: &[Value<'_>],
    first: usize,
    canvas: &str,
    variables: &[Variable],
) -> InputChain {
    let mut chain = InputChain::default();
    let mut pending = vec![first];
    while let Some(at) = pending.pop() {
        if !chain.operations.insert(at) {
            continue;
        }
        let Some(operation) = operations.get(at).and_then(reader::as_struct) else {
            chain.unverified_edges += 1;
            continue;
        };
        if let Some(binding) = reader::text(operation, "binding") {
            if variables
                .iter()
                .any(|var| var.in_namespace(canvas) && var.matches(binding))
            {
                chain.supplied_variables.insert(at);
            } else {
                chain.absent_variables.insert(at);
            }
        }
        operation.visit_owned_values(|visit| match visit.value() {
            Value::Reference(reference) => {
                trace_reference(
                    owner,
                    reference,
                    InputSite {
                        operation: at,
                        value_ordinal: visit.ordinal(),
                    },
                    operations.len(),
                    &mut pending,
                    &mut chain,
                );
            }
            Value::Error(_) => chain.unverified_edges += 1,
            _ => {}
        });
    }
    chain
}

fn trace_reference(
    owner: &RecordTree<'_>,
    reference: &Reference<'_>,
    from: InputSite,
    operation_count: usize,
    pending: &mut Vec<usize>,
    chain: &mut InputChain,
) {
    let location = reference.location();
    if !matches!(
        location.and_then(|value| value.steps().first()),
        Some(Step::Field("operations"))
    ) {
        return;
    }
    match bindings::index_at(location, "operations") {
        Some(at) if reference.record_index() == owner.record_index() && at < operation_count => {
            chain.edges.insert(LocalInput { from, target: at });
            pending.push(at);
        }
        _ => chain.unverified_edges += 1,
    }
}

#[cfg(test)]
mod tests {
    use p4kx_dataforge::DataForge;
    use p4kx_testkit::{
        DataForgeBuilder, TestCanvas, TestOp, TestWidget, entity_schema, screens_schema, tag_record,
    };

    use super::{BindingEvidence, BindingInputs};
    use crate::report::Report;
    use crate::{Variable, WidgetInput, WidgetSection};

    #[test]
    fn the_direct_target_keeps_intermediate_operations_and_variable_availability()
    -> Result<(), &'static str> {
        let bytes = binding_fixture();
        let forge = DataForge::open(&bytes).map_err(|_| "cannot open synthetic DataForge")?;
        let widget = WidgetInput {
            record_index: 0,
            section: WidgetSection::Scene,
            slot: 0,
        };
        let variable =
            Variable::new("variable", "true").map_err(|_| "invalid synthetic variable")?;
        for (variables, supplied, absent) in
            [(std::slice::from_ref(&variable), 1, 0), (&[][..], 0, 1)]
        {
            let mut evidence = BindingEvidence::default();
            evidence.inspect(
                &BindingInputs::new(&forge, variables, &Report::default()),
                widget,
            )?;
            let [site] = evidence.sites.as_slice() else {
                return Err("expected one original binding site");
            };
            assert!(
                evidence.sites().eq([(widget, 2)]),
                "original target and binding coordinates"
            );
            assert_eq!(
                site.field.as_deref(),
                Some("PrimaryStateTag"),
                "authored field"
            );
            assert!(
                site.direct_target,
                "the original widget Reference selects the parent"
            );
            assert!(
                site.input.operations.iter().copied().eq([0, 1, 2]),
                "original nested operations"
            );
            assert!(
                site.input
                    .edges
                    .iter()
                    .map(|edge| (edge.from.operation, edge.target))
                    .eq([(1, 0), (2, 1)]),
                "original edge sources and targets"
            );
            let counts = evidence.counts();
            assert_eq!(counts.supplied, supplied, "supplied declarations");
            assert_eq!(counts.absent, absent, "absent declarations");
            assert_eq!(counts.source_gaps, 0, "all source references are readable");
        }
        Ok(())
    }

    fn binding_fixture() -> Vec<u8> {
        let canvas = TestCanvas::new(0x5a31, "binding_trace", 8.0, 8.0)
            .widget(TestWidget::display("parent"))
            .operation("flag", TestOp::BoolVar)
            .operation("pick", TestOp::TagFromBool("flag", 0x5a32, 0x5a33))
            .operation(
                "bind",
                TestOp::TagField("parent", "PrimaryStateTag", "pick"),
            );
        screens_schema(entity_schema(DataForgeBuilder::new()))
            .record(canvas.record())
            .record(tag_record(0x5a32, "On"))
            .record(tag_record(0x5a33, "Off"))
            .build()
    }
}

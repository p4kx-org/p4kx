// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::{ControlFlow, Range};

use crate::build::{element_bytes, pointer, words};
use crate::header;
use crate::query::instance_source;
use crate::query::reader::{Elements, Select, Walk};
use crate::query::sink::{Reached, Sink};
use crate::query::source::{QueryByteUse, QuerySource, TraversalInput};
use crate::query::traversal_gap;
use crate::query::walk::{HEADER_LEN, Place, deref, derives};
use crate::{DataForge, Value, ValueError, ValueKind};

impl Walk {
    pub(super) fn array<'f, S: Sink<'f>>(
        &self,
        forge: &'f DataForge<'f>,
        steps: Range<usize>,
        place: Place<'f>,
        (at, elements, select): (u64, &Elements, &Select),
        sink: &mut S,
    ) -> Result<ControlFlow<()>, ValueError> {
        let (next, end) = (steps.start, steps.end);
        let index = next.saturating_sub(1);
        let header = place.field(forge, at, HEADER_LEN);
        let selected_header = sink
            .source_depth()
            .and_then(|_| header.as_ref().ok())
            .and_then(|bytes| forge.byte_span(bytes))
            .map(|span| span.start);
        let bytes = match header {
            Ok(bytes) => bytes,
            Err(error) => return Ok(missing_array_header(forge, place, at, error, sink)),
        };
        sink.route_bytes(forge, bytes, QueryByteUse::Array);
        sink.follow_decision(index);
        let [count, first] = words(bytes);
        if let Err(error) = check_range(forge, elements, first, count, sink) {
            sink.observe_input_bytes(
                forge,
                bytes,
                QueryByteUse::AttemptedTraversal(TraversalInput::Array),
            );
            return Ok(sink.reached(Reached::Value(Value::Error(error))));
        }
        let indices = match select {
            Select::Index(element) if *element < count => *element..element.saturating_add(1),
            Select::Index(_) => return Ok(sink.reached(Reached::Value(Value::Null))),
            Select::All | Select::Type(_) => 0..count,
        };
        sink.charge(indices.len())?;
        for element in indices {
            if sink.proceed().is_break() {
                return Ok(ControlFlow::Break(()));
            }
            let depth = sink.source_depth();
            if let Some(header_start) = selected_header {
                sink.route(QuerySource::ArrayElement {
                    header_start,
                    index: element,
                });
            }
            let reached = element_of(forge, elements, select, first.saturating_add(element), sink);
            sink.follow_decision(index);
            let flow = match reached {
                Some(reached) => self.continue_from(forge, next, end, reached, sink)?,
                None => ControlFlow::Continue(()),
            };
            if let Some(depth) = depth {
                sink.restore_sources(depth);
            }
            if flow.is_break() {
                return Ok(flow);
            }
        }
        Ok(ControlFlow::Continue(()))
    }
}

fn missing_array_header<'f>(
    forge: &'f DataForge<'f>,
    place: Place<'f>,
    at: u64,
    error: ValueError,
    sink: &mut impl Sink<'f>,
) -> ControlFlow<()> {
    let reached = traversal_gap::missing(
        forge,
        place,
        at,
        HEADER_LEN,
        TraversalInput::Array,
        error,
        sink,
    );
    sink.reached(reached)
}

/// Checks that the elements from `first` lie inside the pool or instance block they are read
/// from.
fn check_range<'source>(
    forge: &DataForge<'_>,
    elements: &Elements,
    first: u32,
    count: u32,
    sink: &mut impl Sink<'source>,
) -> Result<(), ValueError> {
    let (len, pool) = match elements {
        Elements::Values(kind) => value_pool_bound(forge, *kind, sink),
        Elements::Pointers(_) => value_pool_bound(forge, ValueKind::StrongPointer, sink),
        Elements::Instances(target) => {
            if let Some(mut observe) = sink.input_observer() {
                observe(QuerySource::InstanceBlockBound { structure: *target });
            }
            let Some(&count) = forge.instance_counts.get(*target as usize) else {
                return Err(ValueError::UnknownStruct {
                    index: u16::try_from(*target).unwrap_or(u16::MAX),
                });
            };
            (u64::from(count), "instance block")
        }
    };
    if u64::from(first).saturating_add(u64::from(count)) > len {
        return Err(ValueError::ArrayOutOfRange {
            first,
            count,
            len,
            pool,
        });
    }
    Ok(())
}

fn value_pool_bound<'source>(
    forge: &DataForge<'_>,
    kind: ValueKind,
    sink: &mut impl Sink<'source>,
) -> (u64, &'static str) {
    if let Some(mut observe) = sink.input_observer()
        && let Some(span) = header::value_pool_count_span(kind)
    {
        observe(QuerySource::Bytes {
            start: span.start,
            end: span.end,
            kind: QueryByteUse::PoolBound,
        });
    }
    (forge.value_count(kind) as u64, kind.name())
}

/// Returns what the element at `element` in its pool or block reaches, or `None` when
/// `[TypeName]` does not select it.
fn element_of<'f>(
    forge: &'f DataForge<'f>,
    elements: &Elements,
    select: &Select,
    element: u32,
    sink: &mut impl Sink<'f>,
) -> Option<Reached<'f>> {
    let wanted = match select {
        Select::Type(wanted) => Some(&wanted[..]),
        Select::Index(_) | Select::All => None,
    };
    Some(match elements {
        Elements::Values(kind) => Reached::Place(Place::Element {
            kind: *kind,
            element,
            bytes: element_bytes(forge, *kind, element),
        }),
        Elements::Instances(target) => {
            sink.route(QuerySource::Structure { index: *target });
            if wanted.is_some_and(|wanted| !derives(forge, *target, wanted, sink)) {
                return None;
            }
            let instance = instance_source::read(forge, *target, element, sink.input_observer());
            instance.map_or_else(
                || {
                    Reached::Value(Value::Error(ValueError::InvalidPointer {
                        struct_index: *target,
                        instance_index: element,
                    }))
                },
                |bytes| {
                    sink.route(QuerySource::Instance {
                        structure: *target,
                        instance: element,
                    });
                    Reached::Place(Place::Instance {
                        struct_index: *target,
                        bytes,
                    })
                },
            )
        }
        Elements::Pointers(declared) => {
            let bytes = element_bytes(forge, ValueKind::StrongPointer, element);
            sink.route_bytes(forge, bytes, QueryByteUse::Pointer);
            if let Some(wanted) = wanted {
                let (struct_index, instance_index) = pointer(bytes)?;
                sink.route(QuerySource::Structure {
                    index: struct_index,
                });
                if forge.instance(struct_index, instance_index).is_some()
                    && !derives(forge, struct_index, wanted, sink)
                {
                    if let Some(observe) = sink.input_observer() {
                        instance_source::extent(struct_index, instance_index, true, observe);
                    }
                    return None;
                }
            }
            deref(forge, bytes, declared, sink)
        }
    })
}

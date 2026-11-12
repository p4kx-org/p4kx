// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::{ControlFlow, Range};

use crate::build::{element_bytes, pointer, words};
use crate::query::reader::{Elements, Select, Walk};
use crate::query::sink::{Reached, Sink};
use crate::query::source::{QueryByteUse, QuerySource};
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
        let [count, first] = match header {
            Ok(bytes) => {
                sink.route_bytes(forge, bytes, QueryByteUse::Array);
                sink.follow_decision(index);
                words(bytes)
            }
            Err(error) => return Ok(sink.reached(Reached::Value(Value::Error(error)))),
        };
        if let Err(error) = check_range(forge, elements, first, count) {
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

/// Checks that the elements from `first` lie inside the pool or instance block they are read
/// from.
fn check_range(
    forge: &DataForge<'_>,
    elements: &Elements,
    first: u32,
    count: u32,
) -> Result<(), ValueError> {
    let (len, pool) = match elements {
        Elements::Values(kind) => (forge.value_count(*kind) as u64, kind.name()),
        Elements::Pointers(_) => (
            forge.value_count(ValueKind::StrongPointer) as u64,
            ValueKind::StrongPointer.name(),
        ),
        Elements::Instances(target) => match forge.instance_counts.get(*target as usize) {
            Some(count) => (u64::from(*count), "instance block"),
            None => {
                return Err(ValueError::UnknownStruct {
                    index: u16::try_from(*target).unwrap_or(u16::MAX),
                });
            }
        },
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
            if wanted.is_some_and(|wanted| !derives(forge, *target, wanted)) {
                return None;
            }
            forge.instance(*target, element).map_or(
                Reached::Value(Value::Error(ValueError::InvalidPointer {
                    struct_index: *target,
                    instance_index: element,
                })),
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
                    && !derives(forge, struct_index, wanted)
                {
                    return None;
                }
            }
            deref(forge, bytes, declared, sink)
        }
    })
}

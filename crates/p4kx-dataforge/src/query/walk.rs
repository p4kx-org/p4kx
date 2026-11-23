// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use crate::build::{pointer, reference_guid};
use crate::query::instance_source;
use crate::query::reader::{Link, Step, Walk};
use crate::query::schema::{AncestorStep, ancestors_include, ancestors_include_observed};
use crate::query::sink::{Live, Reached, Sink};
use crate::query::source::{CellTrace, QueryByteUse, QuerySource, TraversalInput};
use crate::query::state::WalkState;
use crate::query::traversal_gap;
use crate::record::RecordNameKind;
use crate::{DataForge, Value, ValueError, ValueKind};

/// The bytes a path reads at one point: an instance of a struct or one value of a pool.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Place<'f> {
    /// An instance of the struct at `struct_index`.
    Instance { struct_index: u32, bytes: &'f [u8] },
    /// The value at `element` in the pool of `kind`.
    Element {
        kind: ValueKind,
        element: u32,
        bytes: &'f [u8],
    },
}

impl<'f> Place<'f> {
    /// Returns the bytes of the instance or value.
    pub(crate) const fn bytes(&self) -> &'f [u8] {
        match self {
            Self::Instance { bytes, .. } | Self::Element { bytes, .. } => bytes,
        }
    }

    /// Returns the error that the bytes end before `needed`.
    pub(crate) fn too_short(&self, forge: &DataForge<'_>, needed: u64) -> ValueError {
        match *self {
            Self::Instance {
                struct_index,
                bytes,
            } => ValueError::InstanceTooShort {
                struct_index,
                size: bytes.len() as u64,
                needed,
            },
            Self::Element { kind, element, .. } => ValueError::ArrayOutOfRange {
                first: element,
                count: 1,
                len: forge.value_count(kind) as u64,
                pool: kind.name(),
            },
        }
    }

    /// Returns the `len` bytes at `at`, or the error that the place ends before them.
    pub(super) fn field(
        &self,
        forge: &DataForge<'_>,
        at: u64,
        len: u64,
    ) -> Result<&'f [u8], ValueError> {
        let end = at.checked_add(len);
        end.and_then(|end| {
            let range = usize::try_from(at).ok()?..usize::try_from(end).ok()?;
            self.bytes().get(range)
        })
        .ok_or_else(|| self.too_short(forge, end.unwrap_or(u64::MAX)))
    }
}

/// The bytes of a pointer and of an array header.
pub(super) const HEADER_LEN: u64 = 8;

impl Walk {
    /// Calls `emit` with each value the path reaches in order; a null pointer or an index out of
    /// range reaches null and an unreadable step an error value.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::PathTooWide`] when the paths of the Record reach more than
    /// [`MAX_PATH_VALUES`](crate::query::MAX_PATH_VALUES) elements together.
    pub(super) fn visit<'f>(
        &self,
        forge: &'f DataForge<'f>,
        place: Place<'f>,
        state: &mut WalkState,
        emit: &mut dyn FnMut(Value<'f>) -> ControlFlow<()>,
        trace: Option<&mut CellTrace>,
    ) -> Result<(), ValueError> {
        let last_follow = trace.as_ref().and_then(|_| self.last_follow());
        let mut live = Live {
            forge,
            leaf: &self.leaf,
            state,
            emit,
            trace,
            last_follow,
        };
        self.step(forge, 0, self.steps.len(), place, &mut live)
            .map(|_| ())
    }

    /// Walks the steps from `index` up to `end` and gives the sink what the last of them reaches.
    pub(super) fn step<'f, S: Sink<'f>>(
        &self,
        forge: &'f DataForge<'f>,
        index: usize,
        end: usize,
        place: Place<'f>,
        sink: &mut S,
    ) -> Result<ControlFlow<()>, ValueError> {
        let depth = sink.source_depth();
        let flow = self.step_inner(forge, index, end, place, sink);
        if let Some(depth) = depth {
            sink.restore_sources(depth);
        }
        flow
    }

    fn step_inner<'f, S: Sink<'f>>(
        &self,
        forge: &'f DataForge<'f>,
        index: usize,
        end: usize,
        place: Place<'f>,
        sink: &mut S,
    ) -> Result<ControlFlow<()>, ValueError> {
        if sink.proceed().is_break() {
            return Ok(ControlFlow::Break(()));
        }
        let step = match self.steps.get(index) {
            Some(step) if index < end => step,
            _ => return Ok(sink.reached(Reached::Place(place))),
        };
        let next = index.saturating_add(1);
        let (at, elements, select) = match step {
            Step::Pointer { at, declared } => {
                let reached = match place.field(forge, *at, HEADER_LEN) {
                    Ok(bytes) => {
                        sink.route_bytes(forge, bytes, QueryByteUse::Pointer);
                        deref(forge, bytes, declared, sink)
                    }
                    Err(error) => traversal_gap::missing(
                        forge,
                        place,
                        *at,
                        HEADER_LEN,
                        TraversalInput::Pointer,
                        error,
                        sink,
                    ),
                };
                sink.follow_decision(index);
                return self.continue_from(forge, next, end, reached, sink);
            }
            Step::Follow { at, link, declared } => {
                let reached = match place.field(forge, *at, link.len()) {
                    Ok(bytes) => {
                        let kind = if *link == Link::Weak {
                            QueryByteUse::WeakLink
                        } else {
                            QueryByteUse::Reference
                        };
                        sink.route_bytes(forge, bytes, kind);
                        follow(forge, *link, bytes, declared, sink)
                    }
                    Err(error) => traversal_gap::missing(
                        forge,
                        place,
                        *at,
                        link.len(),
                        link.input(),
                        error,
                        sink,
                    ),
                };
                sink.follow_decision(index);
                return self.continue_from(forge, next, end, reached, sink);
            }
            Step::Array {
                at,
                elements,
                select,
            } => (*at, elements, select),
        };
        self.array(forge, next..end, place, (at, elements, select), sink)
    }

    /// Continues at the step `next` from what a pointer or link reached.
    pub(super) fn continue_from<'f, S: Sink<'f>>(
        &self,
        forge: &'f DataForge<'f>,
        next: usize,
        end: usize,
        reached: Reached<'f>,
        sink: &mut S,
    ) -> Result<ControlFlow<()>, ValueError> {
        match reached {
            Reached::Place(target) => self.step(forge, next, end, target, sink),
            Reached::Value(value) => Ok(sink.reached(Reached::Value(value))),
        }
    }
}

impl Link {
    /// Returns which original link slot is addressed.
    const fn input(self) -> TraversalInput {
        match self {
            Self::Reference => TraversalInput::Reference,
            Self::Weak => TraversalInput::WeakLink,
        }
    }

    /// Returns the number of bytes the link takes in an instance or pool.
    const fn len(self) -> u64 {
        match self {
            Self::Reference => ValueKind::Reference.element_len() as u64,
            Self::Weak => HEADER_LEN,
        }
    }
}

/// Follows a Reference to the instance of its Record, or a weak pointer to the instance it
/// names, when that is an instance of `declared` or of a struct deriving from it; a null or
/// dangling Reference and a null weak pointer reach null and are counted in `state`.
fn follow<'f>(
    forge: &'f DataForge<'f>,
    link: Link,
    bytes: &'f [u8],
    declared: &[usize],
    sink: &mut impl Sink<'f>,
) -> Reached<'f> {
    if link == Link::Weak {
        let Some((structure, index)) = pointer(bytes) else {
            return unresolved(sink, bytes);
        };
        let reached = pointed_instance(forge, structure, index, declared, sink);
        traversal_gap::readable(forge, bytes, link.input(), &reached, sink);
        return reached;
    }
    let Some(guid) = reference_guid(bytes) else {
        return unresolved(sink, bytes);
    };
    let record = forge.record_by_guid(guid);
    if let Some(observe) = sink.predicate_observer() {
        observe(QuerySource::RecordLookup {
            kind: RecordNameKind::Guid,
            end: forge.record_count() as u64,
        });
    }
    let Some(record) = record else {
        sink.predicate_bytes(forge, bytes, QueryByteUse::DanglingReference);
        return unresolved(sink, bytes);
    };
    let index = record.index();
    if let Some(observe) = sink.predicate_observer() {
        observe(QuerySource::ReferenceTarget {
            index,
            kind: RecordNameKind::Guid,
        });
        observe(QuerySource::RecordType { index });
    }
    sink.route(QuerySource::Record { index });
    let struct_index = record.def.struct_index;
    let reached = instance(
        forge,
        struct_index,
        u32::from(record.def.instance_index),
        declared,
        |declared| ValueError::UnexpectedReferenceType {
            guid: record.guid(),
            struct_index,
            declared,
        },
        sink,
    );
    traversal_gap::readable(forge, bytes, link.input(), &reached, sink);
    reached
}

/// Counts the link in `bytes` in the sink and returns the null it reaches.
fn unresolved<'f>(sink: &mut impl Sink<'f>, bytes: &'f [u8]) -> Reached<'f> {
    sink.unresolved(bytes);
    Reached::Value(Value::Null)
}

/// Follows a strong pointer to an instance of `declared` or of a struct deriving from it.
pub(super) fn deref<'f>(
    forge: &'f DataForge<'f>,
    bytes: &[u8],
    declared: &[usize],
    sink: &mut impl Sink<'f>,
) -> Reached<'f> {
    let Some((struct_index, instance_index)) = pointer(bytes) else {
        return Reached::Value(Value::Null);
    };
    let reached = pointed_instance(forge, struct_index, instance_index, declared, sink);
    traversal_gap::readable(forge, bytes, TraversalInput::Pointer, &reached, sink);
    reached
}

fn pointed_instance<'f>(
    forge: &'f DataForge<'f>,
    struct_index: u32,
    instance_index: u32,
    declared: &[usize],
    sink: &mut impl Sink<'f>,
) -> Reached<'f> {
    instance(
        forge,
        struct_index,
        instance_index,
        declared,
        |declared| ValueError::UnexpectedPointerType {
            struct_index,
            declared,
        },
        sink,
    )
}

/// Reaches the instance when it exists and is of `declared` or of a struct deriving from it;
/// otherwise the error value, with `mismatch` of the declared struct for another struct.
fn instance<'f>(
    forge: &'f DataForge<'f>,
    struct_index: u32,
    instance_index: u32,
    declared: &[usize],
    mismatch: impl FnOnce(u32) -> ValueError,
    sink: &mut impl Sink<'f>,
) -> Reached<'f> {
    let target = instance_source::read(
        forge,
        struct_index,
        instance_index,
        sink.predicate_observer(),
    );
    let Some(target) = target else {
        return Reached::Value(Value::Error(ValueError::InvalidPointer {
            struct_index,
            instance_index,
        }));
    };
    sink.route(QuerySource::Structure {
        index: struct_index,
    });
    if !derives(forge, struct_index, declared, sink) {
        let declared = declared.first().copied().unwrap_or_default();
        return Reached::Value(Value::Error(mismatch(
            u32::try_from(declared).unwrap_or(u32::MAX),
        )));
    }
    sink.route(QuerySource::Instance {
        structure: struct_index,
        instance: instance_index,
    });
    Reached::Place(Place::Instance {
        struct_index,
        bytes: target,
    })
}

pub(super) fn derives<'source>(
    forge: &'source DataForge<'source>,
    struct_index: u32,
    targets: &[usize],
    sink: &impl Sink<'source>,
) -> bool {
    let Ok(index) = usize::try_from(struct_index) else {
        return false;
    };
    let Some(observe) = sink.predicate_observer() else {
        return ancestors_include(forge, index, targets);
    };
    ancestors_include_observed(forge, index, targets, |step| {
        observe(match step {
            AncestorStep::Parent {
                index,
                stored_parent,
            } => QuerySource::AncestorParent {
                index,
                stored_parent,
            },
            AncestorStep::Unavailable { index } => QuerySource::AncestorGap { index },
        });
    })
}

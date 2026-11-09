// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use crate::build::{element_bytes, pointer, reference_guid, words};
use crate::query::reader::{Elements, Link, Select, Step, Walk};
use crate::query::schema::ancestors_include;
use crate::query::sink::{Live, Reached, Sink};
use crate::query::state::WalkState;
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
    fn field(&self, forge: &DataForge<'_>, at: u64, len: u64) -> Result<&'f [u8], ValueError> {
        let end = at.checked_add(len);
        end.and_then(|end| {
            let range = usize::try_from(at).ok()?..usize::try_from(end).ok()?;
            self.bytes().get(range)
        })
        .ok_or_else(|| self.too_short(forge, end.unwrap_or(u64::MAX)))
    }
}

/// The bytes of a pointer and of an array header.
const HEADER_LEN: u64 = 8;

impl Walk {
    /// Calls `emit` with each value the path reaches in order; a null pointer or an index out of
    /// range reaches null and an unreadable step an error value.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::PathTooWide`] when the paths of the Record reach more than
    /// [`MAX_PATH_VALUES`](crate::query::MAX_PATH_VALUES) elements together.
    pub(crate) fn visit<'f>(
        &self,
        forge: &'f DataForge<'f>,
        place: Place<'f>,
        state: &mut WalkState,
        emit: &mut dyn FnMut(Value<'f>) -> ControlFlow<()>,
    ) -> Result<(), ValueError> {
        let mut live = Live {
            forge,
            leaf: &self.leaf,
            state,
            emit,
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
                    Ok(bytes) => deref(forge, bytes, declared),
                    Err(error) => Reached::Value(Value::Error(error)),
                };
                return self.continue_from(forge, next, end, reached, sink);
            }
            Step::Follow { at, link, declared } => {
                let reached = match place.field(forge, *at, link.len()) {
                    Ok(bytes) => follow(forge, *link, bytes, declared, sink),
                    Err(error) => Reached::Value(Value::Error(error)),
                };
                return self.continue_from(forge, next, end, reached, sink);
            }
            Step::Array {
                at,
                elements,
                select,
            } => (*at, elements, select),
        };
        let header = place.field(forge, at, HEADER_LEN);
        let [count, first] = match header {
            Ok(bytes) => words(bytes),
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
            let flow = match element_of(forge, elements, select, first.saturating_add(element)) {
                Some(Reached::Place(target)) => self.step(forge, next, end, target, sink)?,
                Some(Reached::Value(value)) => sink.reached(Reached::Value(value)),
                None => ControlFlow::Continue(()),
            };
            if flow.is_break() {
                return Ok(flow);
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    /// Continues at the step `next` from what a pointer or link reached.
    fn continue_from<'f, S: Sink<'f>>(
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
    bytes: &[u8],
    declared: &[usize],
    sink: &mut impl Sink<'f>,
) -> Reached<'f> {
    if link == Link::Weak {
        return pointer(bytes).map_or_else(
            || unresolved(sink, bytes),
            |(struct_index, instance_index)| {
                instance(forge, struct_index, instance_index, declared, |declared| {
                    ValueError::UnexpectedPointerType {
                        struct_index,
                        declared,
                    }
                })
            },
        );
    }
    let Some(record) = reference_guid(bytes).and_then(|guid| forge.record_by_guid(guid)) else {
        return unresolved(sink, bytes);
    };
    let struct_index = record.def.struct_index;
    instance(
        forge,
        struct_index,
        u32::from(record.def.instance_index),
        declared,
        |declared| ValueError::UnexpectedReferenceType {
            guid: record.guid(),
            struct_index,
            declared,
        },
    )
}

/// Counts the link in `bytes` in the sink and returns the null it reaches.
fn unresolved<'f>(sink: &mut impl Sink<'f>, bytes: &[u8]) -> Reached<'f> {
    sink.unresolved(bytes);
    Reached::Value(Value::Null)
}

/// Follows a strong pointer to an instance of `declared` or of a struct deriving from it.
fn deref<'f>(forge: &'f DataForge<'f>, bytes: &[u8], declared: &[usize]) -> Reached<'f> {
    let Some((struct_index, instance_index)) = pointer(bytes) else {
        return Reached::Value(Value::Null);
    };
    instance(forge, struct_index, instance_index, declared, |declared| {
        ValueError::UnexpectedPointerType {
            struct_index,
            declared,
        }
    })
}

/// Reaches the instance when it exists and is of `declared` or of a struct deriving from it;
/// otherwise the error value, with `mismatch` of the declared struct for another struct.
fn instance<'f>(
    forge: &'f DataForge<'f>,
    struct_index: u32,
    instance_index: u32,
    declared: &[usize],
    mismatch: impl FnOnce(u32) -> ValueError,
) -> Reached<'f> {
    let Some(target) = forge.instance(struct_index, instance_index) else {
        return Reached::Value(Value::Error(ValueError::InvalidPointer {
            struct_index,
            instance_index,
        }));
    };
    if !derives(forge, struct_index, declared) {
        let declared = declared.first().copied().unwrap_or_default();
        return Reached::Value(Value::Error(mismatch(
            u32::try_from(declared).unwrap_or(u32::MAX),
        )));
    }
    Reached::Place(Place::Instance {
        struct_index,
        bytes: target,
    })
}

fn derives(forge: &DataForge<'_>, struct_index: u32, targets: &[usize]) -> bool {
    usize::try_from(struct_index).is_ok_and(|index| ancestors_include(forge, index, targets))
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
            if wanted.is_some_and(|wanted| !derives(forge, *target, wanted)) {
                return None;
            }
            forge.instance(*target, element).map_or(
                Reached::Value(Value::Error(ValueError::InvalidPointer {
                    struct_index: *target,
                    instance_index: element,
                })),
                |bytes| {
                    Reached::Place(Place::Instance {
                        struct_index: *target,
                        bytes,
                    })
                },
            )
        }
        Elements::Pointers(declared) => {
            let bytes = element_bytes(forge, ValueKind::StrongPointer, element);
            if let Some(wanted) = wanted {
                let (struct_index, instance_index) = pointer(bytes)?;
                if forge.instance(struct_index, instance_index).is_some()
                    && !derives(forge, struct_index, wanted)
                {
                    return None;
                }
            }
            deref(forge, bytes, declared)
        }
    })
}

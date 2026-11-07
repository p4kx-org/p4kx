// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

use crate::dataforge::out_of_range;
use crate::layout::{FieldSlot, Layout, Layouts};
use crate::scalar::{first_bytes, read_scalar};
use crate::{
    DataForge, DataForgeError, DataType, Field, Guid, Location, Record, RecordTree, Step,
    StructValue, Value, ValueError, ValueKind,
};

/// The deepest nesting of owned structs inside a Record that is read; deeper ones are errors.
pub const MAX_NESTING_DEPTH: usize = 64;

/// The most values, fields and array elements together, that one Record tree may hold.
pub const MAX_RECORD_VALUES: usize = 1 << 20;

const NULL: u32 = u32::MAX;

/// A range of the position arena.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// A weak pointer waiting for the Record tree to be complete.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pending {
    pub(crate) target: (u32, u32),
    pub(crate) at: Span,
}

/// Builds canonical Record trees and keeps its scratch space between them.
///
/// One builder per thread builds many Records without repeating the work on their struct
/// layouts; [`Record::tree`] uses a new one each time.
#[derive(Debug, Default)]
pub struct TreeBuilder<'f> {
    read_observer: Option<crate::read_observer::ReadObserver>,
    forge_id: usize,
    pub(crate) layouts: Layouts<'f>,
    pub(crate) owned: HashMap<(u32, u32), Span>,
    pub(crate) positions: Vec<u32>,
    stack: Vec<u32>,
    pub(crate) weak: Vec<Pending>,
    pub(crate) locations: HashMap<(u32, u32), Arc<Location<'f>>>,
    pub(crate) steps: Vec<Step<'f>>,
    value_count: usize,
}

impl<'f> TreeBuilder<'f> {
    /// Creates a builder that observes each actual canonical read before returning it.
    #[must_use]
    pub fn with_read_observer(observer: Arc<dyn crate::CanonicalReadObserver>) -> Self {
        let mut builder = Self::new();
        builder.set_read_observer(Some(observer));
        builder
    }

    /// Sets the observer for subsequent reads without changing cached layout state.
    pub fn set_read_observer(&mut self, observer: Option<Arc<dyn crate::CanonicalReadObserver>>) {
        self.read_observer = observer.map(crate::read_observer::ReadObserver);
    }
    /// Creates a builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the canonical tree of `record`, as [`Record::tree`] describes it.
    ///
    /// # Errors
    ///
    /// Returns the source error of [`Record::tree`] or [`DataForgeError::ObservationStopped`]
    /// when the configured observer stops the caller.
    pub fn build(&mut self, record: &Record<'f>) -> Result<RecordTree<'f>, DataForgeError> {
        let outcome = self.build_tree(record);
        if let Some(observer) = &self.read_observer
            && observer.0.observe(record, outcome.as_ref()).is_break()
        {
            return Err(DataForgeError::ObservationStopped);
        }
        outcome
    }

    fn build_tree(&mut self, record: &Record<'f>) -> Result<RecordTree<'f>, DataForgeError> {
        let forge = record.forge;
        self.start(forge);
        let def = record.def;
        let instance = u32::from(def.instance_index);
        let layout = self.layouts.get(forge, def.struct_index)?;
        let record_path = record.record_path()?;
        let name = record.name()?;
        let tag = record.tag()?.filter(|tag| !tag.is_empty());
        let bytes = forge.instance(def.struct_index, instance).ok_or_else(|| {
            out_of_range("Record", record.index as usize, "instance", instance.into())
        })?;
        self.claim(def.struct_index, instance)?;
        let fields = self.fields(forge, &layout, bytes, 0)?;
        let mut tree = RecordTree {
            record_index: record.index(),
            guid: Guid::from_stored(def.guid),
            record_path,
            name,
            tag,
            root: StructValue {
                type_name: layout.type_name,
                fields,
            },
        };
        self.resolve_weak(forge, &mut tree);
        Ok(tree)
    }

    fn start(&mut self, forge: &'f DataForge<'f>) {
        let id = ptr::from_ref(forge).addr();
        if self.forge_id != id {
            self.layouts.clear();
            self.forge_id = id;
        }
        self.owned.clear();
        self.positions.clear();
        self.stack.clear();
        self.weak.clear();
        self.locations.clear();
        self.value_count = 0;
    }

    /// Marks an instance as owned at the current position; returns `false` when it already is.
    fn claim(&mut self, struct_index: u32, instance_index: u32) -> Result<bool, DataForgeError> {
        if self.owned.contains_key(&(struct_index, instance_index)) {
            return Ok(false);
        }
        let span = self.here()?;
        self.owned.insert((struct_index, instance_index), span);
        Ok(true)
    }

    /// Copies the current position into the arena, which counts against the value limit.
    fn here(&mut self) -> Result<Span, DataForgeError> {
        self.count(self.stack.len())?;
        let start = self.positions.len();
        self.positions.extend_from_slice(&self.stack);
        Ok(Span {
            start,
            end: self.positions.len(),
        })
    }

    const fn count(&mut self, values: usize) -> Result<(), DataForgeError> {
        self.value_count = self.value_count.saturating_add(values);
        if self.value_count > MAX_RECORD_VALUES {
            return Err(DataForgeError::RecordTooLarge {
                limit: MAX_RECORD_VALUES,
            });
        }
        Ok(())
    }

    fn fields(
        &mut self,
        forge: &'f DataForge<'f>,
        layout: &Layout<'f>,
        bytes: &[u8],
        depth: usize,
    ) -> Result<Vec<Field<'f>>, DataForgeError> {
        self.count(layout.fields.len())?;
        let mut fields = Vec::with_capacity(layout.fields.len());
        for (index, slot) in (0_u32..).zip(layout.fields.iter()) {
            let field = bytes.get(slot.start..slot.end).unwrap_or_default();
            self.stack.push(index);
            let value = self.value(forge, slot, field, depth);
            self.stack.pop();
            fields.push(Field {
                name: slot.name,
                value: value?,
            });
        }
        Ok(fields)
    }

    fn value(
        &mut self,
        forge: &'f DataForge<'f>,
        slot: &FieldSlot<'f>,
        bytes: &[u8],
        depth: usize,
    ) -> Result<Value<'f>, DataForgeError> {
        if slot.array {
            return self.array(forge, slot, bytes, depth);
        }
        if slot.data_type == DataType::Class {
            return self.nested(forge, u32::from(slot.target), bytes, depth + 1);
        }
        self.single(forge, slot.data_type, slot.target, bytes, depth)
    }

    /// Reads one value that is no inline class from its stored bytes.
    fn single(
        &mut self,
        forge: &'f DataForge<'f>,
        data_type: DataType,
        target: u16,
        bytes: &[u8],
        depth: usize,
    ) -> Result<Value<'f>, DataForgeError> {
        Ok(match data_type {
            DataType::StrongPointer => match pointer(bytes) {
                None => Value::Null,
                Some((struct_index, instance)) => {
                    return self.owned(forge, struct_index, instance, depth + 1);
                }
            },
            DataType::WeakPointer => self.weak(forge, bytes)?,
            DataType::Reference => reference(forge, bytes),
            other => read_scalar(forge, other, target, bytes).unwrap_or(Value::Null),
        })
    }

    fn nested(
        &mut self,
        forge: &'f DataForge<'f>,
        struct_index: u32,
        bytes: &[u8],
        depth: usize,
    ) -> Result<Value<'f>, DataForgeError> {
        if depth > MAX_NESTING_DEPTH {
            return Ok(Value::Error(ValueError::TooDeep {
                limit: MAX_NESTING_DEPTH,
            }));
        }
        let layout = self.layouts.get(forge, struct_index)?;
        let fields = self.fields(forge, &layout, bytes, depth)?;
        Ok(Value::Struct(StructValue {
            type_name: layout.type_name,
            fields,
        }))
    }

    /// Reads the instance a strong pointer or a class array owns.
    fn owned(
        &mut self,
        forge: &'f DataForge<'f>,
        struct_index: u32,
        instance_index: u32,
        depth: usize,
    ) -> Result<Value<'f>, DataForgeError> {
        let Some(bytes) = forge.instance(struct_index, instance_index) else {
            return Ok(Value::Error(ValueError::InvalidPointer {
                struct_index,
                instance_index,
            }));
        };
        if depth <= MAX_NESTING_DEPTH && !self.claim(struct_index, instance_index)? {
            return Ok(Value::Error(ValueError::OwnedTwice {
                struct_index,
                instance_index,
            }));
        }
        self.nested(forge, struct_index, bytes, depth)
    }

    /// Reads a weak pointer; its target is resolved once the tree is complete.
    fn weak(&mut self, forge: &DataForge<'_>, bytes: &[u8]) -> Result<Value<'f>, DataForgeError> {
        let Some(target) = pointer(bytes) else {
            return Ok(Value::Null);
        };
        if forge.instance(target.0, target.1).is_none() {
            return Ok(Value::Error(ValueError::InvalidPointer {
                struct_index: target.0,
                instance_index: target.1,
            }));
        }
        let at = self.here()?;
        self.weak.push(Pending { target, at });
        Ok(Value::Null)
    }

    fn array(
        &mut self,
        forge: &'f DataForge<'f>,
        slot: &FieldSlot<'f>,
        bytes: &[u8],
        depth: usize,
    ) -> Result<Value<'f>, DataForgeError> {
        let [count, first] = words(bytes);
        let kind = ValueKind::of(slot.data_type);
        let (len, pool) = match kind {
            Some(kind) => (forge.value_count(kind) as u64, kind.name()),
            None => match forge.instance_counts.get(usize::from(slot.target)) {
                Some(count) => (u64::from(*count), "instance block"),
                None => {
                    return Ok(Value::Error(ValueError::UnknownStruct {
                        index: slot.target,
                    }));
                }
            },
        };
        if u64::from(first) + u64::from(count) > len {
            return Ok(Value::Error(ValueError::ArrayOutOfRange {
                first,
                count,
                len,
                pool,
            }));
        }
        if self.value_count.saturating_add(count as usize) > MAX_RECORD_VALUES {
            return Ok(Value::Error(ValueError::TooManyValues {
                count,
                limit: MAX_RECORD_VALUES,
            }));
        }
        self.count(count as usize)?;
        let mut items = Vec::with_capacity(count as usize);
        for index in 0..count {
            let element = first.saturating_add(index);
            self.stack.push(index);
            let item = match kind {
                None => self.owned(forge, u32::from(slot.target), element, depth + 1),
                Some(kind) => {
                    let bytes = element_bytes(forge, kind, element);
                    self.single(forge, slot.data_type, slot.target, bytes, depth)
                }
            };
            self.stack.pop();
            items.push(item?);
        }
        Ok(Value::Array(items.into_boxed_slice()))
    }
}

pub(crate) fn element_bytes<'f>(forge: &DataForge<'f>, kind: ValueKind, element: u32) -> &'f [u8] {
    let len = kind.element_len();
    let start = (element as usize).checked_mul(len);
    let end = start.and_then(|start| start.checked_add(len));
    forge
        .values
        .get(kind.index())
        .zip(start.zip(end))
        .and_then(|(pool, (start, end))| pool.get(start..end))
        .unwrap_or_default()
}

/// Returns the two little-endian words at the start of a pointer or an array header.
pub(crate) fn words(bytes: &[u8]) -> [u32; 2] {
    [0, 4].map(|at| u32::from_le_bytes(first_bytes(bytes.get(at..).unwrap_or_default())))
}

/// Returns the struct and instance a pointer names, or `None` for a null pointer.
pub(crate) fn pointer(bytes: &[u8]) -> Option<(u32, u32)> {
    let [struct_index, instance] = words(bytes);
    (struct_index != NULL || instance != NULL).then_some((struct_index, instance))
}

/// Returns the GUID of the Record a Reference names, or `None` for a null Reference; the GUID
/// follows an instance index that is not used.
pub(crate) fn reference_guid(bytes: &[u8]) -> Option<Guid> {
    let guid = Guid::from_stored(first_bytes(bytes.get(4..).unwrap_or_default()));
    (guid != Guid::default()).then_some(guid)
}

/// Reads a Reference as a link to its Record.
fn reference<'f>(forge: &'f DataForge<'f>, bytes: &[u8]) -> Value<'f> {
    let Some(guid) = reference_guid(bytes) else {
        return Value::Null;
    };
    forge.record_by_guid(guid).map_or(
        Value::Error(ValueError::DanglingReference { guid }),
        |record| record.link(),
    )
}

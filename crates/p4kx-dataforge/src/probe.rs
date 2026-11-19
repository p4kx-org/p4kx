// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build::{element_bytes, pointer, words};
use crate::dataforge::out_of_range;
use crate::{DataForge, DataForgeError, DataType, Record, Struct, TreeBuilder, ValueKind};

/// A cheap test whether a Record's strong pointer array holds an instance of one struct, without
/// building the Record tree.
#[derive(Debug, Clone)]
pub struct PointerProbe<'f> {
    field: &'f str,
    struct_indices: Box<[u32]>,
    strict: bool,
}

impl<'f> DataForge<'f> {
    /// Returns a probe for Records whose array field `field` of strong pointers holds an
    /// instance of a struct named `struct_name`, or `None` when no struct of that name has an
    /// instance: then no Record can match and the probe is not needed.
    #[must_use]
    pub fn pointer_probe(&'f self, field: &'f str, struct_name: &str) -> Option<PointerProbe<'f>> {
        let named: Vec<_> = self
            .structs()
            .filter(|structure| structure.name().is_ok_and(|name| name == struct_name))
            .collect();
        if named
            .iter()
            .all(|structure| structure.instance_count() == 0)
        {
            return None;
        }
        Some(PointerProbe {
            field,
            struct_indices: named.iter().map(Struct::index).collect(),
            strict: false,
        })
    }

    /// Returns an exact-name probe that validates each reached strong-pointer slot.
    ///
    /// The probe includes unclassifiable declarations and exact types without instances. It
    /// returns `None` when readable metadata proves there is no possible matching declaration.
    #[must_use]
    pub fn strict_pointer_probe(
        &'f self,
        field: &'f str,
        struct_name: &str,
    ) -> Option<PointerProbe<'f>> {
        let struct_indices: Box<[u32]> = self
            .structs()
            .filter_map(|structure| match structure.name() {
                Ok(name) if name != struct_name => None,
                _ => Some(structure.index()),
            })
            .collect();
        (!struct_indices.is_empty()).then_some(PointerProbe {
            field,
            struct_indices,
            strict: true,
        })
    }
}

impl<'f> TreeBuilder<'f> {
    /// Returns whether the array field of `record` named by `probe` holds a strong pointer to an
    /// existing instance of a struct the probe names, which is what the tree of the Record shows
    /// as a struct of that type. Only the pointer array is read, never the tree; the match is
    /// exact, so an instance of a derived struct does not count. A null pointer, a pointer to a
    /// struct of another name do not match. A legacy probe ignores missing instances, while a
    /// strict probe reports unreadable targets and missing exact-family instances.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeError`] of the Record's struct layout,
    /// [`DataForgeError::IndexOutOfRange`] when the array exceeds the pool. A strict probe
    /// also returns errors for missing owner storage, unclassifiable targets and missing
    /// instances of the exact type.
    pub fn holds(
        &mut self,
        probe: &PointerProbe<'f>,
        record: &Record<'f>,
    ) -> Result<bool, DataForgeError> {
        let forge = record.forge;
        let def = record.def;
        let layout = self.layouts.get(forge, def.struct_index)?;
        let mut cursor = layout.cursor()?;
        let mut found = None;
        while let Some(slot) = cursor.next_slot()? {
            if slot.name == probe.field && slot.array && slot.data_type == DataType::StrongPointer {
                found = Some(slot);
                break;
            }
        }
        let Some(slot) = found else {
            return Ok(false);
        };
        let record_index = record.index as usize;
        let instance = u32::from(def.instance_index);
        let bytes = forge
            .instance(def.struct_index, instance)
            .ok_or_else(|| out_of_range("Record", record_index, "instance", instance.into()))
            .and_then(|owner| {
                owner.get(slot.start..slot.end).ok_or_else(|| {
                    out_of_range(
                        "Record",
                        record_index,
                        "instance byte",
                        slot.start.max(owner.len()) as u64,
                    )
                })
            });
        match bytes {
            Ok(bytes) => array_holds(forge, probe, record_index, words(bytes)),
            Err(error) if probe.strict => Err(error),
            Err(_) => Ok(false),
        }
    }
}

/// Returns whether the elements `first..first + count` of the pool of strong pointers hold a
/// pointer to an existing instance of a struct of `probe`.
fn array_holds(
    forge: &DataForge<'_>,
    probe: &PointerProbe<'_>,
    record: usize,
    [count, first]: [u32; 2],
) -> Result<bool, DataForgeError> {
    let pool = forge.value_count(ValueKind::StrongPointer);
    let end = u64::from(first) + u64::from(count);
    if end > pool as u64 {
        return Err(out_of_range("Record", record, "strong pointer pool", end));
    }
    let mut pointers = (first..first.saturating_add(count)).filter_map(|element| {
        pointer(element_bytes(forge, ValueKind::StrongPointer, element))
            .map(|target| (element, target))
    });
    if probe.strict {
        return strict_array_holds(forge, probe, pointers);
    }
    Ok(pointers.any(|(_, (struct_index, instance))| {
        probe.struct_indices.contains(&struct_index)
            && forge.instance(struct_index, instance).is_some()
    }))
}

fn strict_array_holds(
    forge: &DataForge<'_>,
    probe: &PointerProbe<'_>,
    pointers: impl Iterator<Item = (u32, (u32, u32))>,
) -> Result<bool, DataForgeError> {
    let mut matched = false;
    for (slot, (struct_index, instance)) in pointers {
        let Some(structure) = forge.struct_at(struct_index) else {
            return Err(out_of_range(
                "strong pointer pool",
                slot as usize,
                "struct",
                struct_index.into(),
            ));
        };
        if !probe.struct_indices.contains(&struct_index) {
            continue;
        }
        structure.name()?;
        if forge.instance(struct_index, instance).is_none() {
            return Err(out_of_range(
                "strong pointer pool",
                slot as usize,
                "struct instance",
                instance.into(),
            ));
        }
        matched = true;
    }
    Ok(matched)
}

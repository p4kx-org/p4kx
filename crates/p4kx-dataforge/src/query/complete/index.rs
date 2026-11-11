// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Range;

use crate::query::schema::StructFields;
use crate::rows::NO_PARENT;
use crate::source_layout_allocation;
use crate::source_layout_index;
use crate::{DataForge, DataForgeError, DataType};

/// Owned physical declarations and flat ancestry for completing Queries.
#[derive(Debug, Clone, Default)]
pub struct QuerySchema {
    pub(super) names: Box<[Box<str>]>,
    pub(super) structs: Box<[Entry]>,
    pub(super) types: Box<[usize]>,
    pub(super) all_fields: Box<[Field]>,
    declarations: Box<[Field]>,
    ids: HashMap<Box<str>, usize>,
    first_struct: HashMap<usize, usize>,
}

/// One original struct and its own physical declaration range.
#[derive(Debug, Clone)]
pub(super) struct Entry {
    pub(super) name: Option<usize>,
    pub(super) parent: Option<usize>,
    pub(super) fields: Range<usize>,
    pub(super) valid: bool,
    pub(super) inherited_count: u64,
    pub(super) depth: usize,
}

/// One field declaration used while resolving a completion path.
#[derive(Debug, Clone, Copy)]
pub(super) struct Field {
    pub(super) name: usize,
    pub(super) data_type: DataType,
    pub(super) array: bool,
    pub(super) target: u16,
}

#[derive(Default)]
struct Interner<'source> {
    ids: HashMap<&'source str, usize>,
    names: Vec<Box<str>>,
}

impl<'source> Interner<'source> {
    fn id(&mut self, name: &'source str) -> usize {
        if let Some(id) = self.ids.get(name) {
            return *id;
        }
        let id = self.names.len();
        self.names.push(name.into());
        self.ids.insert(name, id);
        id
    }
}

impl QuerySchema {
    /// Reads owned type names and declarations, retaining no inherited field copies.
    ///
    /// # Errors
    ///
    /// Returns an operational source-layout failure; malformed layouts contribute no fields.
    pub fn of(forge: &DataForge<'_>) -> Result<Self, DataForgeError> {
        let structs = StructFields::read_all(forge)?;
        let mut interner = Interner::default();
        let mut declarations = Vec::new();
        let physical = structs.index.physical_fields();
        source_layout_allocation::reserve(
            &mut declarations,
            physical.len(),
            "completion declarations",
        )?;
        for field in physical {
            declarations.push(Field {
                name: interner.id(field.slot.name(forge)?),
                data_type: field.slot.data_type(),
                array: field.slot.is_array(),
                target: field.slot.target(),
            });
        }
        let entries = entries(&structs, &mut interner)?;
        let names = std::mem::take(&mut interner.names).into_boxed_slice();
        validate(&entries, &declarations, &names)?;
        let mut types: Vec<usize> = entries.iter().filter_map(|entry| entry.name).collect();
        types.sort_unstable_by(|a, b| by_name(name_of(&names, *a), name_of(&names, *b)));
        types.dedup();
        let mut first_struct = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            if let Some(name) = entry.name {
                first_struct.entry(name).or_insert(index);
            }
        }
        let all_fields = super::declarations::union(&entries, &declarations, &names)?;
        let ids = interner
            .ids
            .into_iter()
            .map(|(name, id)| (Box::from(name), id))
            .collect();
        Ok(Self {
            all_fields,
            names,
            structs: entries.into_boxed_slice(),
            types: types.into_boxed_slice(),
            declarations: declarations.into_boxed_slice(),
            ids,
            first_struct,
        })
    }

    /// Returns how many structs the schema lists.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.structs.len()
    }

    /// Returns whether the schema lists no struct.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.structs.is_empty()
    }

    pub(super) fn name(&self, id: usize) -> &str {
        name_of(&self.names, id)
    }
    pub(super) fn entry(&self, index: usize) -> Option<&Entry> {
        self.structs.get(index)
    }

    /// Returns the first struct named `name`.
    pub(super) fn position_of(&self, name: &str) -> Option<usize> {
        self.first_struct.get(self.ids.get(name)?).copied()
    }

    /// Returns the structs with an inherited or own field named `name`.
    pub(super) fn declaring<'query>(
        &'query self,
        name: &'query str,
    ) -> impl Iterator<Item = usize> + 'query {
        (0..self.len()).filter(move |&index| self.field(index, name).is_some())
    }

    /// Returns the first original occurrence, searching each ancestor without copying it.
    pub(super) fn field(&self, index: usize, name: &str) -> Option<Field> {
        if !self.entry(index)?.valid {
            return None;
        }
        let mut current = Some(index);
        let mut found = None;
        while let Some(index) = current {
            let entry = self.entry(index)?;
            if let Some(field) = self
                .own_fields(entry.fields.start, entry.fields.len())
                .find(|field| self.name(field.name) == name)
            {
                found = Some(field);
            }
            current = entry.parent;
        }
        found
    }

    /// Visits own declarations backwards, then each parent's, for first-occurrence replacement.
    pub(super) fn fields(&self, index: usize) -> impl Iterator<Item = Field> + '_ {
        let start = self.entry(index).filter(|entry| entry.valid);
        std::iter::successors(start, |entry| {
            entry.parent.and_then(|parent| self.entry(parent))
        })
        .flat_map(|entry| {
            self.own_fields(entry.fields.start, entry.fields.len())
                .rev()
        })
    }

    /// Iterates an own range validated before the schema becomes available.
    fn own_fields(
        &self,
        first: usize,
        count: usize,
    ) -> impl DoubleEndedIterator<Item = Field> + '_ {
        self.declarations.iter().skip(first).take(count).copied()
    }

    /// Returns whether the struct at `index` is the struct at `base` or derives from it.
    pub(super) fn derives(&self, index: usize, base: usize) -> bool {
        let mut current = index;
        for _ in 0..self.structs.len() {
            if current == base {
                return true;
            }
            match self.entry(current).and_then(|entry| entry.parent) {
                Some(parent) => current = parent,
                None => return false,
            }
        }
        false
    }

    pub(super) fn related(&self, index: usize, other: usize) -> bool {
        self.derives(index, other) || self.derives(other, index)
    }

    pub(super) fn struct_name(&self, index: usize) -> Option<&str> {
        self.entry(index)?.name.map(|id| self.name(id))
    }
}

fn entries<'source>(
    structs: &StructFields<'source>,
    interner: &mut Interner<'source>,
) -> Result<Vec<Entry>, DataForgeError> {
    let mut entries = Vec::new();
    source_layout_allocation::reserve(&mut entries, structs.len(), "completion structs")?;
    for index in 0..structs.len() {
        let name = structs.name(index).map(|name| interner.id(name));
        let row = structs.index.row(index)?;
        let parent = (row.parent != NO_PARENT).then_some(row.parent as usize);
        let (fields, inherited_count, depth, valid) = match structs.source(index) {
            Ok(layout) => (
                layout.summary.fields.clone(),
                layout.summary.inherited_count,
                layout.summary.depth,
                true,
            ),
            Err(error) if source_layout_index::is_operational(&error) => return Err(error),
            Err(_) => (0..0, 0, 0, false),
        };
        entries.push(Entry {
            name,
            parent,
            fields,
            valid,
            inherited_count,
            depth,
        });
    }
    Ok(entries)
}

/// Checks owned ranges and references before lookup iterators become available.
fn validate(
    entries: &[Entry],
    declarations: &[Field],
    names: &[Box<str>],
) -> Result<(), DataForgeError> {
    for field in declarations {
        names
            .get(field.name)
            .ok_or(DataForgeError::LayoutInvariant)?;
    }
    for entry in entries {
        declarations
            .get(entry.fields.clone())
            .ok_or(DataForgeError::LayoutInvariant)?;
        if let Some(name) = entry.name {
            names.get(name).ok_or(DataForgeError::LayoutInvariant)?;
        }
        if entry.valid {
            if let Some(parent) = entry.parent {
                let parent = entries.get(parent).ok_or(DataForgeError::LayoutInvariant)?;
                if !parent.valid || parent.depth.checked_add(1) != Some(entry.depth) {
                    return Err(DataForgeError::LayoutInvariant);
                }
            } else if entry.depth != 1 {
                return Err(DataForgeError::LayoutInvariant);
            }
        }
    }
    Ok(())
}

fn name_of(names: &[Box<str>], id: usize) -> &str {
    names.get(id).map_or("", |name| name)
}

/// Orders names ignoring ASCII case, and names equal so by their bytes.
pub(super) fn by_name(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(b.bytes().map(|byte| byte.to_ascii_lowercase()))
        .then_with(|| a.cmp(b))
}

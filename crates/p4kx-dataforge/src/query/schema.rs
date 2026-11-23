// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::layout_cursor::LayoutCursor;
use crate::rows::NO_PARENT;
use crate::source_layout_allocation;
use crate::source_layout_index::{self, SourceLayout, SourceLayoutIndex};
use crate::{DataForge, DataForgeError, DataType, LayoutSource};

pub(crate) use crate::layout_cursor::LayoutField as FieldDef;

/// Returns whether values of `data_type` are scalars a column can hold.
pub(crate) const fn is_scalar(data_type: DataType) -> bool {
    !matches!(
        data_type,
        DataType::Class | DataType::StrongPointer | DataType::WeakPointer | DataType::Reference
    )
}

/// Optional type names and a borrow of the common validated source-layout index.
pub(crate) struct StructFields<'source, 'observer> {
    forge: &'source DataForge<'source>,
    pub(crate) index: &'source SourceLayoutIndex,
    names: Vec<Option<&'source str>>,
    observe: Option<&'observer dyn Fn(LayoutSource)>,
}

impl<'source, 'observer> StructFields<'source, 'observer> {
    pub(crate) fn read_all(forge: &'source DataForge<'source>) -> Result<Self, DataForgeError> {
        let index = forge.source_layouts()?;
        let mut names = Vec::new();
        source_layout_allocation::reserve(&mut names, forge.structs.len(), "query type names")?;
        names.extend(
            forge
                .structs
                .iter()
                .map(|row| forge.blob.get(row.name).ok()),
        );
        Ok(Self {
            forge,
            index,
            names,
            observe: None,
        })
    }

    pub(crate) fn observe_layouts(&mut self, observe: Option<&'observer dyn Fn(LayoutSource)>) {
        self.observe = observe;
    }

    pub(crate) const fn forge(&self) -> &'source DataForge<'source> {
        self.forge
    }
    pub(crate) const fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &'source str> + '_ {
        self.names.iter().flatten().copied()
    }

    pub(crate) fn name(&self, index: usize) -> Option<&'source str> {
        self.names.get(index).copied().flatten()
    }

    pub(crate) fn positions_of<'name>(
        &'name self,
        name: &'name str,
    ) -> impl Iterator<Item = usize> + 'name {
        self.names
            .iter()
            .enumerate()
            .filter(move |(index, candidate)| {
                let candidate = **candidate;
                if let Some(observe) = self.observe {
                    observe(LayoutSource::TypeName {
                        index: *index,
                        name: candidate.and_then(|name| self.forge.original_text_span(name)),
                    });
                }
                candidate == Some(name)
            })
            .map(|(index, _)| index)
    }

    pub(crate) fn position_of(&self, name: &str) -> Option<usize> {
        self.positions_of(name).next()
    }

    pub(super) fn observe_property_target(&self, field: &FieldDef<'_>) {
        if matches!(
            field.slot.data_type(),
            DataType::Enum
                | DataType::Class
                | DataType::StrongPointer
                | DataType::WeakPointer
                | DataType::Reference
        ) && let Some(observe) = self.observe
        {
            observe(LayoutSource::PropertyTarget {
                property_index: field.slot.property_index(),
            });
        }
    }

    pub(super) fn derives_from(&self, index: usize, target: usize) -> bool {
        let Some(observe) = self.observe else {
            return ancestors_include(self.forge, index, &[target]);
        };
        ancestors_include_observed(self.forge, index, &[target], |step| {
            observe(match step {
                AncestorStep::Parent {
                    index,
                    stored_parent,
                } => LayoutSource::TypeParent {
                    index,
                    stored_parent,
                },
                AncestorStep::Unavailable { index } => LayoutSource::UnavailableAncestry { index },
            });
        })
    }

    pub(crate) fn source(&self, index: usize) -> Result<SourceLayout<'source>, DataForgeError> {
        self.index.layout(
            self.forge,
            u32::try_from(index).map_err(|_| DataForgeError::LayoutInvariant)?,
        )
    }

    pub(crate) fn fields(
        &self,
        index: usize,
    ) -> Result<
        impl Iterator<Item = Result<FieldDef<'source>, DataForgeError>> + use<'source, 'observer>,
        DataForgeError,
    > {
        let observe = self.observe;
        let cursor = self.source(index).and_then(|layout| layout.cursor());
        if let (Err(_), Some(observe)) = (&cursor, observe) {
            observe(LayoutSource::UnavailableLayout { index });
        }
        let mut cursor: LayoutCursor<'source> = cursor?;
        if let Some(observe) = observe {
            cursor.observe_lineage(observe)?;
        }
        Ok(std::iter::from_fn(move || match observe {
            Some(observe) => {
                let mut observe = observe;
                cursor.next_with(Some(&mut observe))
            }
            None => cursor.next(),
        }))
    }

    pub(crate) fn field(
        &self,
        index: usize,
        name: &str,
    ) -> Result<Option<FieldDef<'source>>, DataForgeError> {
        for field in self.fields(index)? {
            let field = field?;
            if field.name == name {
                return Ok(Some(field));
            }
        }
        Ok(None)
    }

    pub(crate) fn field_names(
        &self,
        index: usize,
    ) -> Result<impl Iterator<Item = Result<&'source str, DataForgeError>>, DataForgeError> {
        Ok(self
            .fields(index)?
            .map(|field| field.map(|field| field.name)))
    }

    pub(crate) fn all_field_names(
        &self,
    ) -> impl Iterator<Item = Result<&'source str, DataForgeError>> + '_ {
        (0..self.len()).flat_map(|index| {
            let (fields, error) = match self.fields(index) {
                Ok(fields) => (Some(fields), None),
                Err(error) if source_layout_index::is_operational(&error) => (None, Some(error)),
                Err(_) => (None, None),
            };
            fields
                .into_iter()
                .flatten()
                .map(|field| field.map(|field| field.name))
                .chain(error.into_iter().map(Err))
        })
    }
}

/// Returns whether the struct at `index` or one of its ancestors is among `targets`.
pub(crate) fn ancestors_include(forge: &DataForge<'_>, index: usize, targets: &[usize]) -> bool {
    ancestors_include_observed(forge, index, targets, |_| {})
}

#[derive(Clone, Copy)]
pub(super) enum AncestorStep {
    Parent { index: usize, stored_parent: u32 },
    Unavailable { index: usize },
}

pub(super) fn ancestors_include_observed(
    forge: &DataForge<'_>,
    index: usize,
    targets: &[usize],
    mut observe: impl FnMut(AncestorStep),
) -> bool {
    let mut current = index;
    for _ in 0..forge.struct_count() {
        if targets.contains(&current) {
            return true;
        }
        let Some(row) = forge.structs.get(current) else {
            observe(AncestorStep::Unavailable { index: current });
            return false;
        };
        observe(AncestorStep::Parent {
            index: current,
            stored_parent: row.parent,
        });
        if row.parent == NO_PARENT {
            return false;
        }
        let Ok(parent) = usize::try_from(row.parent) else {
            observe(AncestorStep::Unavailable { index: current });
            return false;
        };
        current = parent;
    }
    observe(AncestorStep::Unavailable { index: current });
    false
}

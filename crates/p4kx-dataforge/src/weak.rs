// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem;
use std::sync::Arc;

use crate::build::Span;
use crate::{
    DataForge, Field, Location, RecordTree, Reference, Step, TreeBuilder, Value, ValueError,
};

impl<'f> TreeBuilder<'f> {
    /// Replaces the placeholder of every weak pointer of `tree` with its Reference.
    pub(crate) fn resolve_weak(&mut self, forge: &'f DataForge<'f>, tree: &mut RecordTree<'f>) {
        let positions = mem::take(&mut self.positions);
        let weak = mem::take(&mut self.weak);
        for pending in &weak {
            let value = self.weak_target(forge, tree, pending.target, &positions);
            let slot = value_at_mut(&mut tree.root.fields, span(&positions, pending.at));
            debug_assert!(slot.is_some(), "every weak pointer has its placeholder");
            if let Some(slot) = slot {
                *slot = value;
            }
        }
        self.positions = positions;
        self.weak = weak;
    }

    fn weak_target(
        &mut self,
        forge: &'f DataForge<'f>,
        tree: &RecordTree<'f>,
        target: (u32, u32),
        positions: &[u32],
    ) -> Value<'f> {
        let outside = Value::Error(ValueError::OutsideRecord {
            struct_index: target.0,
            instance_index: target.1,
        });
        let Some(owned) = self.owned.get(&target).copied() else {
            return forge
                .record_at(target.0, target.1)
                .and_then(|index| forge.record(index))
                .map_or(outside, |record| record.link());
        };
        let at = span(positions, owned);
        if at.is_empty() {
            return Value::Reference(Box::new(Reference {
                record_index: tree.record_index(),
                guid: tree.guid,
                record_path: tree.record_path,
                type_name: tree.type_name(),
                location: None,
            }));
        }
        self.steps.clear();
        let Some(type_name) = locate(&tree.root.fields, at, &mut self.steps) else {
            return outside;
        };
        let steps = &self.steps;
        let location = self
            .locations
            .entry(target)
            .or_insert_with(|| Arc::new(Location::new(steps)))
            .clone();
        Value::Reference(Box::new(Reference {
            record_index: tree.record_index(),
            guid: tree.guid,
            record_path: tree.record_path,
            type_name,
            location: Some(location),
        }))
    }
}

fn span(positions: &[u32], span: Span) -> &[u32] {
    positions.get(span.start..span.end).unwrap_or_default()
}

/// Writes the steps to the struct at the position `at` into `steps` and returns its type.
fn locate<'f>(fields: &[Field<'f>], at: &[u32], steps: &mut Vec<Step<'f>>) -> Option<&'f str> {
    let mut fields = fields;
    let mut positions = at.iter();
    let mut type_name = None;
    while let Some(&position) = positions.next() {
        let field = fields.get(position as usize)?;
        steps.push(Step::Field(field.name));
        let mut current = &field.value;
        if let Value::Array(items) = current {
            let index = *positions.next()?;
            steps.push(Step::Element(index));
            current = items.get(index as usize)?;
        }
        let Value::Struct(structure) = current else {
            return None;
        };
        fields = &structure.fields;
        type_name = Some(structure.type_name);
    }
    type_name
}

/// Returns the value at the position `at`: field and element indices from the Record down.
fn value_at_mut<'t, 'f>(fields: &'t mut [Field<'f>], at: &[u32]) -> Option<&'t mut Value<'f>> {
    let (first, rest) = at.split_first()?;
    let mut value = &mut fields.get_mut(*first as usize)?.value;
    for &step in rest {
        value = match value {
            Value::Struct(structure) => &mut structure.fields.get_mut(step as usize)?.value,
            Value::Array(items) => items.get_mut(step as usize)?,
            _ => return None,
        };
    }
    Some(value)
}

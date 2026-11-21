// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem;
use std::sync::Arc;

use crate::build::Positions;

use crate::{
    DataForge, DataForgeError, Field, Location, RecordTree, Reference, Step, TreeBuilder, Value,
    ValueError,
};

impl<'source> TreeBuilder<'source> {
    pub(crate) fn resolve_weak(
        &mut self,
        forge: &'source DataForge<'source>,
        tree: &mut RecordTree<'source>,
    ) -> Result<(), DataForgeError> {
        let positions = mem::take(&mut self.positions);
        let pending = mem::take(&mut self.weak);
        let mut path = mem::take(&mut self.position_scratch);
        let outcome = (|| {
            for weak in &pending {
                let value = self.weak_target(forge, tree, weak.target, &positions, &mut path)?;
                positions.path(weak.at, &mut path)?;
                let slot = value_at_mut(&mut tree.root.fields, &path)
                    .ok_or(DataForgeError::CanonicalTreeInvariant)?;
                *slot = value;
            }
            Ok(())
        })();
        self.positions = positions;
        self.weak = pending;
        self.position_scratch = path;
        outcome
    }

    fn weak_target(
        &mut self,
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        target: (u32, u32),
        positions: &Positions,
        path: &mut Vec<u64>,
    ) -> Result<Value<'source>, DataForgeError> {
        let Some(owned) = self.owned.get(&target).copied() else {
            return Ok(forge
                .record_at(target.0, target.1)
                .and_then(|index| forge.record(index))
                .map_or(
                    Value::Error(ValueError::OutsideRecord {
                        struct_index: target.0,
                        instance_index: target.1,
                    }),
                    |record| record.link(),
                ));
        };
        if owned.is_root() {
            return Ok(reference(tree, tree.type_name(), None));
        }
        if let Some((type_name, location)) = self.locations.get(&target) {
            return Ok(reference(tree, type_name, Some(location.clone())));
        }
        positions.path(owned, path)?;
        self.steps.clear();
        let type_name = locate(&tree.root.fields, path, &mut self.steps)
            .ok_or(DataForgeError::CanonicalTreeInvariant)?;
        let location = Arc::new(Location::original_instance(&self.steps, path));
        self.locations.insert(target, (type_name, location.clone()));
        Ok(reference(tree, type_name, Some(location)))
    }
}

fn reference<'source>(
    tree: &RecordTree<'source>,
    type_name: &'source str,
    location: Option<Arc<Location<'source>>>,
) -> Value<'source> {
    Value::Reference(Box::new(Reference {
        owner: tree.owner,
        record_index: tree.record_index(),
        guid: tree.guid,
        record_path: tree.record_path,
        type_name,
        location,
    }))
}

/// Writes semantic steps for the actual physical tuple retained by the original decoder.
fn locate<'source>(
    fields: &[Field<'source>],
    at: &[u64],
    steps: &mut Vec<Step<'source>>,
) -> Option<&'source str> {
    let mut fields = fields;
    let mut positions = at.iter();
    let mut type_name = None;
    while let Some(&position) = positions.next() {
        let field = fields.get(usize::try_from(position).ok()?)?;
        steps.push(Step::Field(field.name));
        let mut current = &field.value;
        if let Value::Array(items) = current {
            let index = *positions.next()?;
            steps.push(Step::Element(u32::try_from(index).ok()?));
            current = items.get(usize::try_from(index).ok()?)?;
        }
        let Value::Struct(structure) = current else {
            return None;
        };
        fields = &structure.fields;
        type_name = Some(structure.type_name);
    }
    type_name
}

fn value_at_mut<'tree, 'source>(
    fields: &'tree mut [Field<'source>],
    at: &[u64],
) -> Option<&'tree mut Value<'source>> {
    let (first, rest) = at.split_first()?;
    let mut value = &mut fields.get_mut(usize::try_from(*first).ok()?)?.value;
    for &step in rest {
        value = match value {
            Value::Struct(structure) => {
                &mut structure.fields.get_mut(usize::try_from(step).ok()?)?.value
            }
            Value::Array(items) => items.get_mut(usize::try_from(step).ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}

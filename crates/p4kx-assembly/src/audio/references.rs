// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::convert::Infallible;

use p4kx_dataforge::{Location, RecordTree, Step, StructValue, Value};

#[path = "trigger_reference_positions.rs"]
mod positions;
pub use positions::{
    PositionedTriggerReference, TriggerOccurrence, positioned_trigger_references_in,
};

/// The struct through which a Record names an audio trigger.
const AUDIO_STRUCT: &str = "GlobalResourceAudio";

/// The field of [`AUDIO_STRUCT`] that holds the trigger name.
const TRIGGER_FIELD: &str = "audioTrigger";

/// An audio trigger a Record names: the trigger name, where the Record names it first, and how
/// often.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerReference {
    name: Box<str>,
    location: Box<str>,
    count: u32,
}

impl TriggerReference {
    /// Returns the trigger name as the Record first writes it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the location of the first field naming it, e.g.
    /// `Components[3].triggers[0].audioTrigger`.
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Returns how many fields of the Record name it.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }
}

/// Which fields of a Record may name an audio trigger.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TriggerScope {
    /// Only the `audioTrigger` field of a `GlobalResourceAudio` struct.
    GlobalResourceAudio,
    /// The `audioTrigger` field of a `GlobalResourceAudio` struct and every string field of any
    /// other struct whose name contains `audioTrigger`, ignoring ASCII case, e.g. the
    /// `audioTriggerName` of a melee weapon.
    #[default]
    AudioTriggerFields,
}

/// Returns the audio triggers a Record tree names through [`TriggerScope::AudioTriggerFields`]:
/// see [`trigger_references_in`].
#[must_use]
pub fn trigger_references(tree: &RecordTree<'_>) -> Vec<TriggerReference> {
    trigger_references_in(tree, TriggerScope::AudioTriggerFields)
}

/// Returns the audio triggers a Record tree names through the structs `scope` allows.
///
/// Each trigger is listed once, in the order it first appears, names matched ignoring ASCII
/// case; empty trigger names are skipped.
#[must_use]
pub fn trigger_references_in(tree: &RecordTree<'_>, scope: TriggerScope) -> Vec<TriggerReference> {
    let mut walk = Walk {
        scope,
        ..Walk::default()
    };
    match walk.structure(tree.root(), &mut |_| Ok::<(), Infallible>(())) {
        Ok(()) => walk.found,
        Err(never) => match never {},
    }
}

fn names_a_trigger(field: &str) -> bool {
    field
        .as_bytes()
        .windows(TRIGGER_FIELD.len())
        .any(|window| window.eq_ignore_ascii_case(TRIGGER_FIELD.as_bytes()))
}

#[derive(Clone, Copy)]
enum Position {
    Field(usize),
    Element(u32),
}

#[derive(Clone, Copy)]
enum Visit {
    Enter(Position),
    Leave,
    Trigger { index: usize, structure: u32 },
}

#[derive(Default)]
struct Walk<'f> {
    scope: TriggerScope,
    steps: Vec<Step<'f>>,
    found: Vec<TriggerReference>,
    seen: HashMap<Box<str>, usize>,
}

impl<'f> Walk<'f> {
    fn structure<E>(
        &mut self,
        structure: &StructValue<'f>,
        observe: &mut impl FnMut(Visit) -> Result<(), E>,
    ) -> Result<(), E> {
        let audio = structure.type_name() == AUDIO_STRUCT;
        if audio
            && let Some((ordinal, field)) = structure
                .fields()
                .iter()
                .enumerate()
                .find(|(_, field)| field.name() == TRIGGER_FIELD)
            && let Value::String(name) = field.value()
            && !name.is_empty()
        {
            self.scoped(
                Step::Field(TRIGGER_FIELD),
                Position::Field(ordinal),
                observe,
                |walk, observe| walk.trigger(name, structure.struct_index(), observe),
            )?;
        }
        let anywhere = self.scope == TriggerScope::AudioTriggerFields && !audio;
        for (ordinal, field) in structure.fields().iter().enumerate() {
            self.scoped(
                Step::Field(field.name()),
                Position::Field(ordinal),
                observe,
                |walk, observe| {
                    if anywhere
                        && names_a_trigger(field.name())
                        && let Value::String(name) = field.value()
                        && !name.is_empty()
                    {
                        walk.trigger(name, structure.struct_index(), observe)?;
                    }
                    walk.value(field.value(), observe)
                },
            )?;
        }
        Ok(())
    }

    fn value<E>(
        &mut self,
        value: &Value<'f>,
        observe: &mut impl FnMut(Visit) -> Result<(), E>,
    ) -> Result<(), E> {
        match value {
            Value::Struct(structure) => self.structure(structure, observe),
            Value::Array(items) => {
                for (ordinal, item) in (0_u32..).zip(items.iter()) {
                    if matches!(item, Value::Struct(_) | Value::Array(_)) {
                        self.scoped(
                            Step::Element(ordinal),
                            Position::Element(ordinal),
                            observe,
                            |walk, observe| walk.value(item, observe),
                        )?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn scoped<O, E>(
        &mut self,
        step: Step<'f>,
        position: Position,
        observe: &mut O,
        visit: impl FnOnce(&mut Self, &mut O) -> Result<(), E>,
    ) -> Result<(), E>
    where
        O: FnMut(Visit) -> Result<(), E>,
    {
        self.steps.push(step);
        observe(Visit::Enter(position))?;
        visit(self, observe)?;
        observe(Visit::Leave)?;
        self.steps.pop();
        Ok(())
    }

    fn trigger<E>(
        &mut self,
        name: &str,
        structure: u32,
        observe: &mut impl FnMut(Visit) -> Result<(), E>,
    ) -> Result<(), E> {
        let key: Box<str> = name.to_ascii_lowercase().into();
        let index = if let Some(&index) = self.seen.get(&key)
            && let Some(reference) = self.found.get_mut(index)
        {
            reference.count = reference.count.saturating_add(1);
            index
        } else {
            let index = self.found.len();
            self.seen.insert(key, index);
            self.found.push(TriggerReference {
                name: name.into(),
                location: Location::new(&self.steps).as_str().into(),
                count: 1,
            });
            index
        };
        observe(Visit::Trigger { index, structure })
    }
}

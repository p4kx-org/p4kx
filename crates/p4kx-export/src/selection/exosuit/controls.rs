// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ENTITY_TYPE;
use p4kx_extraction::{DataType, Field, Record, RecordTree, StructValue, Value};

use crate::selection::Exosuit;

const ACTOR_INPUTS: [&str; 4] = [
    "SActorComponentParams",
    "SSCItemControllableManagerParams",
    "ItemControlComponentParams",
    "ItemControllerComponentParams",
];
const DEFAULT_LOADOUT: &str = "SEntityComponentDefaultLoadoutParams";
const SEAT: &str = "SCItemSeatParams";
const OPERATOR_MODES: &str = "OperatorModeAvailabilityParams";

pub(super) enum ControlRead<T> {
    Present(T),
    Absent,
    Unreadable(Option<u32>),
}

pub(super) struct Control<'tree, 'source> {
    pub(super) actor: Actor<'tree, 'source>,
    pub(super) seat: Seat<'source>,
}

pub(super) struct Actor<'tree, 'source> {
    pub(super) controllers: [&'tree Value<'source>; 3],
    pub(super) seat_link: &'tree Value<'source>,
    pub(super) entries: &'tree [Value<'source>],
    seat_record: Record<'source>,
}

pub(super) struct Seat<'source> {
    pub(super) record: Record<'source>,
    pub(super) tree: RecordTree<'source>,
}

impl<'tree, 'source> Control<'tree, 'source> {
    pub(super) fn read(exosuit: Exosuit, root: &'tree StructValue<'source>) -> ControlRead<Self> {
        let Some(actor) = Actor::read(exosuit, root) else {
            return if unreadable_actor(root) {
                ControlRead::Unreadable(None)
            } else {
                ControlRead::Absent
            };
        };
        match Seat::read(actor.seat_record) {
            ControlRead::Present(seat) => ControlRead::Present(Self { actor, seat }),
            ControlRead::Absent => ControlRead::Absent,
            ControlRead::Unreadable(index) => ControlRead::Unreadable(index),
        }
    }
}

impl<'tree, 'source> Actor<'tree, 'source> {
    fn read(exosuit: Exosuit, root: &'tree StructValue<'source>) -> Option<Self> {
        if unreadable_components(root) {
            return None;
        }
        let [actor, manager, control, controller] = ACTOR_INPUTS.map(|name| component(root, name));
        actor?;
        let controllers = [manager?, control?, controller?];
        if controllers.iter().any(|(_, component)| {
            !matches!(component.field("enabled"), None | Some(Value::Bool(true)))
        }) {
            return None;
        }
        let (_, loadout) = component(root, DEFAULT_LOADOUT)?;
        let Value::Struct(loadout) = loadout.field("loadout")? else {
            return None;
        };
        let Value::Array(entries) = loadout.field("entries")? else {
            return None;
        };
        if entries.iter().any(unreadable_entry) {
            return None;
        }
        let mut selected = entries.iter().filter_map(|value| entry(exosuit, value));
        let (port, seat_link, seat_record) = selected.next()?;
        if selected.next().is_some()
            || entries
                .iter()
                .filter(|entry| ports_match(entry, port))
                .count()
                != 1
        {
            return None;
        }
        Some(Self {
            controllers: controllers.map(|(value, _)| value),
            seat_link,
            entries,
            seat_record,
        })
    }
}

impl<'source> Seat<'source> {
    fn read(record: Record<'source>) -> ControlRead<Self> {
        let Ok(tree) = record.tree() else {
            return ControlRead::Unreadable(Some(record.index()));
        };
        if unreadable_components(tree.root()) {
            return ControlRead::Unreadable(Some(record.index()));
        }
        if component(tree.root(), SEAT).is_some_and(|(_, seat)| controls_seat(seat, &record)) {
            ControlRead::Present(Self { record, tree })
        } else if component(tree.root(), SEAT).is_some_and(|(_, seat)| unreadable_seat(seat)) {
            ControlRead::Unreadable(Some(record.index()))
        } else {
            ControlRead::Absent
        }
    }

    pub(super) fn component(&self) -> Option<(&Value<'source>, &StructValue<'source>)> {
        component(self.tree.root(), SEAT)
    }
}

fn unreadable_components(root: &StructValue<'_>) -> bool {
    match root.field("Components") {
        Some(Value::Error(_)) => true,
        Some(Value::Array(values)) => values.iter().any(|value| matches!(value, Value::Error(_))),
        _ => false,
    }
}

fn unreadable_actor(root: &StructValue<'_>) -> bool {
    if unreadable_components(root)
        || ACTOR_INPUTS.iter().skip(1).any(|kind| {
            component(root, kind).is_some_and(|(_, controller)| {
                matches!(controller.field("enabled"), Some(Value::Error(_)))
            })
        })
    {
        return true;
    }
    let loadout = component(root, DEFAULT_LOADOUT).and_then(|(_, owner)| owner.field("loadout"));
    match loadout {
        Some(Value::Error(_)) => true,
        Some(Value::Struct(loadout)) => match loadout.field("entries") {
            Some(Value::Error(_)) => true,
            Some(Value::Array(entries)) => entries.iter().any(unreadable_entry),
            _ => false,
        },
        _ => false,
    }
}

fn unreadable_seat(seat: &StructValue<'_>) -> bool {
    if ["seatType", "availableOperatorModes"]
        .iter()
        .any(|name| matches!(seat.field(name), Some(Value::Error(_))))
    {
        return true;
    }
    match seat.field("actorAttachment") {
        Some(Value::Error(_)) => true,
        Some(Value::Struct(attachment)) => {
            matches!(attachment.field("type"), Some(Value::Error(_)))
        }
        _ => false,
    }
}

fn controls_seat(seat: &StructValue<'_>, record: &Record<'_>) -> bool {
    let Some(Value::Struct(attachment)) = seat.field("actorAttachment") else {
        return false;
    };
    attachment.field("type").and_then(text) == Some("Host")
        && seat.field("seatType").and_then(text) == Some("HOTAS_C_L")
        && field(seat, "availableOperatorModes")
            .and_then(|field| record_link(field, OPERATOR_MODES))
            .is_some_and(|target| target.shares_source_with(record))
}

pub(crate) fn supported(tree: &RecordTree<'_>) -> bool {
    Exosuit::from_header(tree.record_path(), tree.name(), tree.type_name()).is_some_and(|exosuit| {
        matches!(Control::read(exosuit, tree.root()), ControlRead::Present(_))
    })
}

pub(crate) fn admit(exosuit: Exosuit, record: &Record<'_>) -> bool {
    let Ok(tree) = record.tree() else {
        return true;
    };
    !matches!(Control::read(exosuit, tree.root()), ControlRead::Absent)
}

fn entry<'tree, 'source>(
    exosuit: Exosuit,
    value: &'tree Value<'source>,
) -> Option<(
    &'tree Value<'source>,
    &'tree Value<'source>,
    Record<'source>,
)> {
    let Value::Struct(entry) = value else {
        return None;
    };
    let port = entry.field("itemPortName")?;
    text(port).filter(|name| !name.trim().is_empty())?;
    let reference = field(entry, "entityClassReference")?;
    let target = record_link(reference, ENTITY_TYPE)?;
    if !exosuit.matches_seat(target.record_path()) {
        return None;
    }
    Some((port, reference.value(), target.record().ok()?))
}

pub(super) fn entry_port<'tree, 'source>(
    value: &'tree Value<'source>,
) -> Option<&'tree Value<'source>> {
    let Value::Struct(entry) = value else {
        return None;
    };
    entry.field("itemPortName")
}

pub(super) fn entry_reference<'tree, 'source>(
    value: &'tree Value<'source>,
) -> Option<&'tree Field<'source>> {
    let Value::Struct(entry) = value else {
        return None;
    };
    field(entry, "entityClassReference")
}

fn unreadable_entry(value: &Value<'_>) -> bool {
    matches!(value, Value::Error(_))
        || matches!(entry_port(value), Some(Value::Error(_)))
        || entry_reference(value).is_some_and(|field| matches!(field.value(), Value::Error(_)))
}

fn ports_match(value: &Value<'_>, port: &Value<'_>) -> bool {
    entry_port(value)
        .and_then(text)
        .zip(text(port))
        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn record_link<'tree, 'source>(
    field: &'tree Field<'source>,
    expected_type: &str,
) -> Option<p4kx_extraction::ReferenceOrigin<'tree, 'source>> {
    if field.storage().data_type() != DataType::Reference {
        return None;
    }
    let Value::Reference(value) = field.value() else {
        return None;
    };
    let origin = value.original_target();
    let declared = field.storage().target();
    if origin.record_type() != Some(expected_type) {
        return None;
    }
    if declared == u16::MAX {
        return Some(origin);
    }
    let mut target = origin.record().ok()?.struct_type().ok()?;
    let declared = u32::from(declared);
    let mut parents = std::collections::HashSet::new();
    while target.index() != declared {
        if !parents.insert(target.index()) {
            return None;
        }
        target = target.parent()?;
    }
    Some(origin)
}

fn field<'tree, 'source>(
    owner: &'tree StructValue<'source>,
    name: &str,
) -> Option<&'tree Field<'source>> {
    owner.fields().iter().find(|field| field.name() == name)
}

fn component<'tree, 'source>(
    root: &'tree StructValue<'source>,
    name: &str,
) -> Option<(&'tree Value<'source>, &'tree StructValue<'source>)> {
    let Value::Array(components) = root.field("Components")? else {
        return None;
    };
    let mut matches = components.iter().filter_map(|value| match value {
        Value::Struct(component) if component.type_name() == name => Some((value, component)),
        _ => None,
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

const fn text<'value>(value: &'value Value<'_>) -> Option<&'value str> {
    match value {
        Value::String(text) | Value::Locale(text) | Value::Enum(text) => Some(text),
        _ => None,
    }
}

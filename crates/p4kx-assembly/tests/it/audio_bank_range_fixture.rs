// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::graph;
use super::{Bank, Object, PLAY, POST};

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Nested,
    ActiveReturn,
    Preference,
    PreferenceAfterOther,
}

pub(crate) fn banks(case: Case) -> Vec<Bank> {
    vec![
        Bank::new("Z-home", home(case)),
        Bank::new("M-first", first(case)),
        Bank::new("A-second", second(case)),
    ]
}

fn event(id: u32, actions: &[u32]) -> Object {
    Object::Event {
        id,
        actions: actions.to_vec(),
    }
}

const fn action(id: u32, kind: u16, target: u32) -> Object {
    Object::Action { id, kind, target }
}

const fn sound(id: u32, wem: u32) -> Object {
    Object::Sound { id, wem }
}

fn terminal(event_id: u32, action_id: u32, sound_id: u32, wem: u32) -> Vec<Object> {
    vec![
        event(event_id, &[action_id]),
        action(action_id, PLAY, sound_id),
        sound(sound_id, wem),
    ]
}

fn home(case: Case) -> Vec<Object> {
    let targets: &[u32] = match case {
        Case::Nested | Case::ActiveReturn => &[200],
        Case::Preference => &[200, 400, 500],
        Case::PreferenceAfterOther => &[200, 300, 400, 500],
    };
    let actions: Vec<_> = (100..).zip(targets).map(|(id, _)| id).collect();
    let mut objects = vec![event(graph::root_event(), &actions)];
    objects.extend(
        (100..)
            .zip(targets)
            .map(|(id, target)| action(id, POST, *target)),
    );
    if matches!(case, Case::Preference | Case::PreferenceAfterOther) {
        objects.extend(terminal(500, 501, 510, 7005));
    }
    objects
}

fn first(case: Case) -> Vec<Object> {
    let mut objects = match case {
        Case::Nested | Case::ActiveReturn => vec![
            event(200, &[201, 202]),
            action(201, POST, 300),
            action(202, PLAY, 210),
            sound(210, 7001),
        ],
        Case::Preference | Case::PreferenceAfterOther => terminal(200, 201, 210, 7001),
    };
    if !matches!(case, Case::Nested) {
        objects.extend(terminal(400, 401, 410, 7003));
    }
    if matches!(case, Case::Preference | Case::PreferenceAfterOther) {
        objects.extend(terminal(500, 511, 520, 7006));
    }
    objects
}

fn second(case: Case) -> Vec<Object> {
    let mut objects = match case {
        Case::Nested | Case::PreferenceAfterOther => terminal(300, 301, 310, 7002),
        Case::ActiveReturn => vec![
            event(300, &[301, 302]),
            action(301, POST, 400),
            action(302, PLAY, 310),
            sound(310, 7002),
        ],
        Case::Preference => Vec::new(),
    };
    if matches!(case, Case::Preference | Case::PreferenceAfterOther) {
        objects.extend(terminal(400, 402, 420, 7004));
    }
    objects
}

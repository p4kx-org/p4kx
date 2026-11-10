// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "../support/audio_bank_graph.rs"]
mod graph;

pub(crate) use graph::{Bank, EVENT, Object, PLAY, POST, TRIGGER, root_event};

pub(crate) const ROOT_ACTION: u32 = 100;
pub(crate) const SECOND_ROOT_ACTION: u32 = 101;
pub(crate) const FIRST_EVENT: u32 = 200;
pub(crate) const FIRST_ACTION: u32 = 201;
pub(crate) const FIRST_SOUND: u32 = 210;
pub(crate) const SECOND_EVENT: u32 = 300;
pub(crate) const SECOND_ACTION: u32 = 301;
pub(crate) const SECOND_SOUND: u32 = 310;
pub(crate) const MISSING_EVENT: u32 = 404;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    ThreeBanks,
    EqualAncestor,
    TrueCycle,
    Ordinary,
    Missing,
}

pub(crate) fn banks(case: Case) -> Vec<Bank> {
    let mut banks = vec![Bank::new("Z-home", home_objects(case))];
    if !matches!(case, Case::Missing) {
        banks.push(first(case));
    }
    if matches!(case, Case::ThreeBanks) {
        banks.push(Bank::new(
            "B-second",
            vec![
                Object::Event {
                    id: SECOND_EVENT,
                    actions: vec![SECOND_ACTION],
                },
                Object::Action {
                    id: SECOND_ACTION,
                    kind: PLAY,
                    target: SECOND_SOUND,
                },
                Object::Sound {
                    id: SECOND_SOUND,
                    wem: 7002,
                },
            ],
        ));
    }
    banks
}

fn home_objects(case: Case) -> Vec<Object> {
    let actions = if matches!(case, Case::ThreeBanks) {
        vec![ROOT_ACTION, SECOND_ROOT_ACTION]
    } else {
        vec![ROOT_ACTION]
    };
    let target = if matches!(case, Case::Missing) {
        MISSING_EVENT
    } else {
        FIRST_EVENT
    };
    let mut objects = vec![
        Object::Event {
            id: root_event(),
            actions,
        },
        Object::Action {
            id: ROOT_ACTION,
            kind: POST,
            target,
        },
    ];
    if matches!(case, Case::ThreeBanks) {
        objects.push(Object::Action {
            id: SECOND_ROOT_ACTION,
            kind: POST,
            target: SECOND_EVENT,
        });
    }
    objects
}

fn first(case: Case) -> Bank {
    let action = if matches!(case, Case::EqualAncestor) {
        ROOT_ACTION
    } else {
        FIRST_ACTION
    };
    let mut objects = vec![Object::Event {
        id: FIRST_EVENT,
        actions: vec![action],
    }];
    if matches!(case, Case::TrueCycle) {
        objects.push(Object::Action {
            id: action,
            kind: POST,
            target: root_event(),
        });
    } else {
        objects.push(Object::Action {
            id: action,
            kind: PLAY,
            target: FIRST_SOUND,
        });
        objects.push(Object::Sound {
            id: FIRST_SOUND,
            wem: 7001,
        });
    }
    Bank::new("A-first", objects)
}

#[path = "audio_bank_range_fixture.rs"]
pub(crate) mod ranges;

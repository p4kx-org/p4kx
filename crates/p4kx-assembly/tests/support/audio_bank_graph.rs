// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};

pub(crate) const TRIGGER: &str = "Original_Bank_Route";
pub(crate) const EVENT: &str = "Original_Bank_Route_Event";
pub(crate) const PLAY: u16 = 0x0403;
pub(crate) const POST: u16 = 0x2103;

pub(crate) enum Object {
    Event { id: u32, actions: Vec<u32> },
    Action { id: u32, kind: u16, target: u32 },
    Sound { id: u32, wem: u32 },
}

impl Object {
    pub(crate) const fn id(&self) -> u32 {
        match self {
            Self::Event { id, .. } | Self::Action { id, .. } | Self::Sound { id, .. } => *id,
        }
    }

    fn encoded(&self) -> TestHircObject {
        match self {
            Self::Event { id, actions } => TestHircObject::event(*id, actions),
            Self::Action { id, kind, target } => TestHircObject::action(*id, *kind, *target),
            Self::Sound { id, wem } => TestHircObject::sound(*id, *wem, 2, 0),
        }
    }
}

pub(crate) struct Bank {
    pub(crate) path: String,
    pub(crate) id: u32,
    pub(crate) objects: Vec<Object>,
    pub(crate) bytes: Vec<u8>,
}

impl Bank {
    pub(crate) fn new(name: &str, objects: Vec<Object>) -> Self {
        let id = p4kx_testkit::wwise_short_id(name);
        let encoded = objects.iter().fold(TestBank::new(id), |bank, object| {
            bank.object(object.encoded())
        });
        Self {
            path: format!("Data\\Sounds\\wwise\\{name}.bnk"),
            id,
            objects,
            bytes: encoded.bytes(),
        }
    }
}

pub(crate) fn root_event() -> u32 {
    p4kx_testkit::wwise_short_id(EVENT)
}

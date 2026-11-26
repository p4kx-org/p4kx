// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserializer;
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};

use super::{Encryption, Inventory, Role, UseFacts, UseIndex};

#[derive(Clone, Copy, Default)]
pub(in super::super) struct Observed {
    encrypted: bool,
    unknown: bool,
    opaque: bool,
}

impl Observed {
    pub(in super::super) const fn marker(self, complete: bool) -> Encryption {
        if self.encrypted {
            Encryption::Encrypted
        } else if complete && !self.unknown {
            Encryption::Unencrypted
        } else {
            Encryption::Unknown
        }
    }

    pub(in super::super) const fn has_opaque(self) -> bool {
        self.opaque
    }

    pub(super) const fn merge(self, other: Self) -> Self {
        Self {
            encrypted: self.encrypted || other.encrypted,
            unknown: self.unknown || other.unknown,
            opaque: self.opaque || other.opaque,
        }
    }

    fn record(&mut self, facts: UseFacts) -> Result<(), &'static str> {
        if !facts.contributes() {
            return Err("noncontributing use in a sourced value");
        }
        self.encrypted |= facts.encryption() == Encryption::Encrypted;
        self.unknown |= facts.encryption() == Encryption::Unknown;
        self.opaque |= facts.opaque();
        Ok(())
    }
}

pub(in super::super) struct References<'inventory>(&'inventory Inventory);

impl<'inventory> References<'inventory> {
    pub(in super::super) const fn new(inventory: &'inventory Inventory) -> Self {
        Self(inventory)
    }
}

impl<'de> DeserializeSeed<'de> for References<'_> {
    type Value = Observed;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Observed, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for References<'_> {
    type Value = Observed;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sorted source-use reference array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Observed, A::Error> {
        let mut observed = Observed::default();
        let mut previous: Option<Box<str>> = None;
        while let Some(id) = sequence.next_element::<Box<str>>()? {
            let facts =
                lookup(&self.0.uses, previous.as_deref(), &id).map_err(de::Error::custom)?;
            observed.record(facts).map_err(de::Error::custom)?;
            previous = Some(id);
        }
        Ok(observed)
    }
}

pub(super) fn contributors(
    uses: &UseIndex,
    references: &[Box<str>],
) -> Result<Observed, &'static str> {
    let mut observed = Observed::default();
    let mut previous = None;
    for reference in references {
        observed.record(lookup(uses, previous, reference)?)?;
        previous = Some(reference.as_ref());
    }
    Ok(observed)
}

pub(super) fn check_attempts(uses: &UseIndex, references: &[Box<str>]) -> Result<(), &'static str> {
    let mut previous = None;
    for reference in references {
        if lookup(uses, previous, reference)?.role != Role::Attempted {
            return Err("variant failed source use does not have the attempted role");
        }
        previous = Some(reference.as_ref());
    }
    Ok(())
}

fn lookup(uses: &UseIndex, previous: Option<&str>, id: &str) -> Result<UseFacts, &'static str> {
    if previous.is_some_and(|previous| previous >= id) {
        return Err("source-use references must be sorted and unique");
    }
    uses.get(id).ok_or("missing referenced source use")
}

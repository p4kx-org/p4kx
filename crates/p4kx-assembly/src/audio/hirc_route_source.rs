// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{BankPlayBankDecision, HircObject};

use super::walk_banks::Loaded;

const EVENT: &str = "bnkHircEvent";
const ACTION: &str = "bnkHircAction";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct HircRouteSource {
    binding: ObservedEntryBinding,
    entry_position: usize,
    version: Option<Range<usize>>,
    words: Box<[HircRouteWord]>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct HircRouteWord {
    pub(super) namespace: &'static str,
    pub(super) field: &'static str,
    pub(super) bytes: Range<u64>,
    pub(super) ordinal: Option<usize>,
}

impl HircRouteWord {
    const fn selector(namespace: &'static str, field: &'static str, bytes: Range<u64>) -> Self {
        Self {
            namespace,
            field,
            bytes,
            ordinal: None,
        }
    }
}

impl HircRouteSource {
    pub(super) fn event_action(
        archive: &Archive,
        owner: &Loaded<'_>,
        object: &HircObject,
        ordinal: usize,
    ) -> Option<Self> {
        let count = object.event_action_count_evidence()?;
        let words = Box::new([
            HircRouteWord::selector(EVENT, "actionCount", count.range.clone()),
            HircRouteWord {
                namespace: EVENT,
                field: "actionId",
                bytes: object.event_action_id_byte_range(ordinal)?,
                ordinal: Some(ordinal),
            },
        ]);
        Self::bound(archive, owner, words)
    }

    pub(super) fn action(
        archive: &Archive,
        owner: &Loaded<'_>,
        object: &HircObject,
        bank_id: Option<u32>,
    ) -> Option<Self> {
        let fields = object.action_evidence()?;
        let action_type = HircRouteWord::selector(
            ACTION,
            "actionType",
            fields.action_type.as_ref()?.range.clone(),
        );
        let target_id =
            HircRouteWord::selector(ACTION, "targetId", fields.target_id.as_ref()?.range.clone());
        let words: Box<[_]> = match (bank_id, &fields.play_bank) {
            (Some(id), BankPlayBankDecision::Play(play)) => {
                let bank_id = play.bank_id.as_ref().filter(|word| word.value == id)?;
                Box::new([
                    action_type,
                    target_id,
                    HircRouteWord::selector(ACTION, "playBankId", bank_id.range.clone()),
                ])
            }
            _ => Box::new([action_type, target_id]),
        };
        Self::bound(archive, owner, words)
    }

    fn bound(archive: &Archive, owner: &Loaded<'_>, words: Box<[HircRouteWord]>) -> Option<Self> {
        let entry_position = owner.soundbank.entry_position();
        let source = archive.observed_entry(archive.entries().get(entry_position)?)?;
        Some(Self {
            binding: source.binding(),
            entry_position,
            version: owner.bank.version_byte_range(),
            words,
        })
    }

    pub(super) fn entry<'a>(&self, archive: &'a Archive) -> Option<ObservedEntry<'a>> {
        let entry = archive.observed_entry(archive.entries().get(self.entry_position)?)?;
        self.binding
            .is_same_entry(&entry.binding())
            .then_some(entry)
    }

    pub(super) fn version_bytes(&self) -> Option<Range<usize>> {
        self.version.clone()
    }

    pub(super) fn words(&self) -> &[HircRouteWord] {
        &self.words
    }
}

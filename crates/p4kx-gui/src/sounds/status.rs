// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::provenance::ResolvedSoundList;
use super::view::{IndexInfo, SoundList, TriggerList};

/// Where the audio index of the Sounds tab stands.
#[derive(Debug, Clone, Copy)]
pub enum SoundsStatus<'a> {
    /// The audio was not asked for yet.
    Idle,
    /// The ATL definitions and soundbanks are indexed off the UI thread.
    Building,
    /// The index is built.
    Ready(&'a IndexInfo),
    /// Indexing failed for the reason given, or stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so its audio was not indexed.
    ArchiveChanged,
}

/// Where the trigger list stands.
#[derive(Debug, Clone, Copy)]
pub enum ListStatus<'a> {
    /// Nothing was asked for.
    Empty,
    /// The list is being made; the previous one stays shown meanwhile.
    Loading,
    /// The Record asked for is no Entity.
    NotAnEntity,
    /// Making the list failed for the reason given.
    Failed(&'a str),
    /// The Archive changed on disk, so the list was not made.
    ArchiveChanged,
    /// The list is made.
    Ready(&'a TriggerList),
}

/// Where the sounds of the selected trigger stand.
#[derive(Debug, Clone, Copy)]
pub enum SoundsListStatus<'a> {
    /// No trigger is selected.
    None,
    /// The sounds are being resolved.
    Loading,
    /// No ATL definition has the trigger.
    Undefined,
    /// The sounds are resolved.
    Ready(&'a SoundList),
}

#[derive(Debug, Default)]
pub(super) enum Stage {
    #[default]
    Idle,
    Building,
    Ready(IndexInfo),
    Failed(Option<String>),
    ArchiveChanged,
}

#[derive(Debug, Default)]
pub(super) enum Sounds {
    #[default]
    None,
    Loading,
    Undefined,
    Ready(Box<ResolvedSoundList>),
}

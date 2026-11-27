// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Archive;
use p4kx_audio::{MediaStorage, SoundSource};

use super::wem_location_origin::WemLocationSource;
use super::{AudioIndex, IndexedSoundbank, LanguageChoice, WemLocation, WemSelection};

/// The selected WEM location and its actual admission rule.
#[derive(Debug, Default)]
pub(crate) struct Located<'i> {
    pub(crate) location: Option<WemLocation<'i>>,
    triplet_offset: Option<u64>,
    pub(crate) selection: WemSelection,
    pub(crate) localised: bool,
    pub(crate) language: Option<&'i str>,
    pub(crate) fallback: bool,
    pub(crate) missing_localisation: bool,
}

impl Located<'_> {
    pub(super) fn source(
        &self,
        index: &AudioIndex,
        archive: &Archive,
    ) -> Option<WemLocationSource> {
        WemLocationSource::capture(index, archive, self.location.as_ref()?, self.triplet_offset)
    }
}

struct Selected<'i> {
    location: WemLocation<'i>,
    triplet_offset: Option<u64>,
    reason: WemSelection,
}

/// Locates the WEM of one Sound object and retains the rule that admitted it.
pub(crate) fn locate<'i>(
    index: &'i AudioIndex,
    soundbank: &'i IndexedSoundbank,
    choice: &LanguageChoice,
    source: &SoundSource,
) -> Located<'i> {
    if !source.has_media_file() {
        return Located::default();
    }
    if matches!(source.storage, MediaStorage::Other(_)) {
        return Located {
            selection: WemSelection::UnsupportedStorage,
            ..Located::default()
        };
    }
    let localised = source.is_language_specific() || soundbank.language().is_some();
    let id = source.source_id;
    let find_loose = || loose(index, soundbank, choice, id, localised);
    let find_embedded = || embedded(index, soundbank, id);
    let selected = match source.storage {
        MediaStorage::Streamed => find_loose(),
        MediaStorage::PrefetchStreamed => find_loose().or_else(find_embedded),
        MediaStorage::Embedded => find_embedded().or_else(find_loose),
        MediaStorage::Other(_) => None,
    };
    let (location, triplet_offset, selection) =
        selected.map_or((None, None, WemSelection::NotFound), |selected| {
            (
                Some(selected.location),
                selected.triplet_offset,
                selected.reason,
            )
        });
    let streamed = matches!(
        source.storage,
        MediaStorage::Streamed | MediaStorage::PrefetchStreamed
    );
    let missing_localisation =
        localised && streamed && !matches!(location, Some(WemLocation::Entry { .. }));
    let language = location.as_ref().and_then(WemLocation::language);
    Located {
        fallback: localised && choice.is_fallback(language),
        language: language.filter(|_| localised),
        location,
        triplet_offset,
        selection,
        localised,
        missing_localisation,
    }
}

const fn same(language: Option<&str>, wanted: Option<&str>) -> bool {
    match (language, wanted) {
        (None, None) => true,
        (Some(language), Some(wanted)) => language.eq_ignore_ascii_case(wanted),
        _ => false,
    }
}

fn loose<'i>(
    index: &'i AudioIndex,
    soundbank: &IndexedSoundbank,
    choice: &LanguageChoice,
    id: u32,
    localised: bool,
) -> Option<Selected<'i>> {
    let pick = |wanted: Option<&str>, reason| {
        index
            .loose()
            .find(id)
            .find(|(language, _)| same(*language, wanted))
            .map(|(_, location)| Selected {
                location,
                triplet_offset: None,
                reason,
            })
    };
    let neutral = || pick(None, WemSelection::LooseNeutral);
    if !localised {
        return neutral();
    }
    let named = || {
        choice
            .preferred()
            .and_then(|wanted| pick(Some(wanted), WemSelection::LoosePreferredLanguage))
            .or_else(|| {
                choice
                    .fallback()
                    .and_then(|wanted| pick(Some(wanted), WemSelection::LooseFallbackLanguage))
            })
    };
    if let Some(language) = soundbank.language() {
        return pick(Some(language), WemSelection::LooseBankLanguage)
            .or_else(named)
            .or_else(neutral);
    }
    if choice.is_any() {
        return index
            .loose()
            .find(id)
            .find(|(language, _)| language.is_some())
            .map(|(_, location)| Selected {
                location,
                triplet_offset: None,
                reason: WemSelection::LooseFirstLocalised,
            })
            .or_else(neutral);
    }
    named().or_else(neutral)
}

fn embedded<'i>(
    index: &'i AudioIndex,
    soundbank: &'i IndexedSoundbank,
    wem: u32,
) -> Option<Selected<'i>> {
    let find = |accepts: &dyn Fn(&IndexedSoundbank) -> bool, reason| {
        index
            .embedded_wem_with_triplet_offsets(wem)
            .find(|(location, _)| matches!(location, WemLocation::Soundbank { soundbank, .. } if accepts(soundbank)))
            .map(|(location, triplet)| Selected { location, triplet_offset: Some(triplet), reason })
    };
    find(
        &|bank| std::ptr::eq(bank, soundbank),
        WemSelection::EmbeddedSameBank,
    )
    .or_else(|| {
        find(
            &|bank| {
                bank.language()
                    .zip(soundbank.language())
                    .is_some_and(|(theirs, ours)| theirs.eq_ignore_ascii_case(ours))
            },
            WemSelection::EmbeddedBankLanguage,
        )
    })
    .or_else(|| {
        find(
            &|bank| bank.language().is_none(),
            WemSelection::EmbeddedNeutral,
        )
    })
    .or_else(|| find(&|_| true, WemSelection::EmbeddedFirstAvailable))
}

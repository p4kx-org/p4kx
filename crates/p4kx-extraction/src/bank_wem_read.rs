// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sound::WemOriginPlan;
use crate::wem_payload_evidence::PayloadPlan;
use crate::{AudioError, EmbeddedWem, MediaClass, Wem};

/// Which complete bank media to submit to the WEM parser.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum BankWemReadMode {
    /// Parse all items other than prefetched prefixes.
    AllNonPrefetched,
    /// Parse candidates after excluding MIDI and plug-in data.
    SoundCandidates,
}

/// The actual result of inspecting one embedded media item.
enum BankWemOutcome<'media> {
    /// Its entire declared WEM does not fit in the retained prefix.
    Prefetch,
    /// Its class excludes it from WEM parsing under the selected mode.
    NonSound,
    /// The original embedded bytes parsed successfully.
    Parsed(Wem<&'media [u8]>),
    /// The attempted WEM parse failed.
    Unreadable(AudioError),
}

/// One physical media item and the outcome of reading its own bytes.
pub struct BankWemRead<'media> {
    wem: EmbeddedWem<'media>,
    known_class: Option<MediaClass>,
    outcome: BankWemOutcome<'media>,
}

impl<'media> BankWemRead<'media> {
    /// Reads one WEM or records why its bytes were not parsed.
    #[must_use]
    pub fn read(wem: EmbeddedWem<'media>, mode: BankWemReadMode) -> Self {
        let known_class = match mode {
            BankWemReadMode::AllNonPrefetched => None,
            BankWemReadMode::SoundCandidates => Some(wem.class()),
        };
        let outcome = if wem.is_prefetch() {
            BankWemOutcome::Prefetch
        } else if known_class.is_some_and(MediaClass::is_non_sound) {
            BankWemOutcome::NonSound
        } else {
            match wem.parse() {
                Ok(parsed) => BankWemOutcome::Parsed(parsed),
                Err(error) => BankWemOutcome::Unreadable(error),
            }
        };
        Self {
            wem,
            known_class,
            outcome,
        }
    }

    /// Returns the original embedded bytes and their physical placement.
    #[must_use]
    pub const fn media(&self) -> &EmbeddedWem<'media> {
        &self.wem
    }

    /// Returns the media class from the original item prefix.
    #[must_use]
    pub fn class(&self) -> MediaClass {
        self.known_class.unwrap_or_else(|| self.wem.class())
    }

    /// Returns whether the admitted item holds only a prefix of the WEM.
    #[must_use]
    pub const fn is_prefetch(&self) -> bool {
        matches!(self.outcome, BankWemOutcome::Prefetch)
    }

    /// Returns the parsed WEM when the original item was decoded.
    #[must_use]
    pub const fn parsed(&self) -> Option<&Wem<&'media [u8]>> {
        match &self.outcome {
            BankWemOutcome::Parsed(parsed) => Some(parsed),
            _ => None,
        }
    }

    /// Returns the error of an attempted WEM parse.
    #[must_use]
    pub const fn error(&self) -> Option<&AudioError> {
        match &self.outcome {
            BankWemOutcome::Unreadable(error) => Some(error),
            _ => None,
        }
    }

    pub(super) fn payload_plan(&self) -> PayloadPlan {
        match &self.outcome {
            BankWemOutcome::Prefetch => PayloadPlan::embedded_failure(&self.wem, None),
            BankWemOutcome::NonSound => PayloadPlan::non_sound(&self.wem),
            BankWemOutcome::Parsed(parsed) => PayloadPlan::parsed(
                &WemOriginPlan::new(parsed.info_source(), parsed.codec(), parsed.sample_count()),
                self.wem.bytes().len() as u64,
                Some(&self.wem),
            ),
            BankWemOutcome::Unreadable(error) => {
                PayloadPlan::embedded_failure(&self.wem, Some(error))
            }
        }
    }
}

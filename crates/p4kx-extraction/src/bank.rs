// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded soundbank reads, source inspection, and extraction.

#[path = "voice_property.rs"]
mod voice_property;

pub use self::voice_property::{VoiceProperty, VoicePropertyName};

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use p4kx_audio::{AudioError, EmbeddedWem, MediaClass, Soundbank};
pub use p4kx_audio::{
    BankActionFields, BankActionRead, BankContainerChild, BankContainerChildren,
    BankContainerFields, BankContainerRead, BankDeclaredLengthSource, BankDidx, BankDidxEntry,
    BankEvent, BankEventAction, BankEventCountEncoding, BankEventRead, BankHirc, BankHircAction,
    BankHircContainer, BankHircEvent, BankHircField, BankHircMusicSegment, BankHircMusicTrack,
    BankHircObject, BankHircSemantic, BankHircSound, BankHircState, BankMediaClassSource,
    BankMediaInfo, BankMediaSourceFields, BankMusicMarker, BankMusicNodeFields,
    BankMusicSegmentFields, BankMusicSegmentRead, BankMusicSegmentRow, BankMusicSegmentRows,
    BankMusicStinger, BankMusicStingerTail, BankMusicTrackClip, BankMusicTrackFields,
    BankMusicTrackRead, BankMusicTrackRow, BankMusicTrackRows, BankMusicTrackSource,
    BankMusicTrackSwitchFields, BankNodeParentFields, BankObservedHeader, BankPlayBankDecision,
    BankPlayBankFields, BankPlayProperties, BankReadError, BankSectionCursor,
    BankSoundContinuation, BankSoundFields, BankSoundNodeBase, BankSoundNodeFields, BankSoundRead,
    BankSource, BankSourceSection, BankStateRead, BankStid, BankStidName, ContainerKind,
    MediaStorage, MusicTrackKind, SoundSource, StateObject, StateProperties, StateProperty,
};
pub use p4kx_audio::{
    BankActorMixerFields, BankHircActionProperties, BankHircActorMixer, BankHircContainerNode,
    BankHircSoundNode, BankNodeAuxFields, BankNodeFxFields, BankNodeFxKind, BankNodeFxList,
    BankNodeFxSlot, BankNodeFxSlots, BankNodePropertyExtent, BankNodePropertyExtents,
    BankNodePropertyKind, BankNodePropertyRow, BankNodePropertyRows, BankNodePropertyValue,
    BankNodeUserAuxBuses,
};
pub use p4kx_audio::{
    BankBlendAssociation, BankBlendAssociationFields, BankBlendAssociations,
    BankBlendContainerFields, BankBlendExtent, BankBlendLayer, BankBlendLayerFields,
    BankBlendLayers, BankBlendPoint, BankBlendPointRows,
};
pub use p4kx_audio::{
    BankDialogueEventFields, BankDialogueEventRow, BankDialogueEventRows, BankDialogueNode,
    BankDialogueTree, BankDialogueTreeCoverage, BankHircDialogueEvent, BankHircMusicSwitch,
    BankMusicSwitchBranch, BankMusicSwitchFields, BankMusicSwitchGroup, BankMusicSwitchNode,
    BankMusicSwitchRead, BankMusicSwitchRow, BankMusicSwitchRows, BankMusicSwitchTarget,
};
pub use p4kx_audio::{
    BankGraphPoint, BankGraphPointRows, BankMusicTrackAutomation, BankMusicTrackAutomationRows,
    BankMusicTrackFadeFields, BankMusicTrackTransitionFields, BankTrackAutomationExtent,
    BankTrackAutomationFields, BankTrackAutomationPoints,
};
pub use p4kx_audio::{
    BankHircMusicPlaylist, BankMusicPlaylistDepth, BankMusicPlaylistFields, BankMusicPlaylistItem,
    BankMusicPlaylistRead, BankMusicPlaylistRow, BankMusicPlaylistRows, BankMusicTransitionFields,
    MusicPlayMode,
};
pub use p4kx_audio::{
    BankHircRtpc, BankRtpcCurve, BankRtpcExtent, BankRtpcPoint, BankRtpcRead, BankRtpcRow,
    BankRtpcRows, BankSharedNode,
};
pub use p4kx_audio::{
    BankHircSoundStates, BankInlineStateRow, BankInlineStateRows, BankSoundStateRead,
    BankStateBinding, BankStateGroup,
};
pub use p4kx_audio::{
    BankMusicFadeFields, BankMusicTransitionDestinationFields, BankMusicTransitionId,
    BankMusicTransitionIdRows, BankMusicTransitionIds, BankMusicTransitionObjectFields,
    BankMusicTransitionRows, BankMusicTransitionRule, BankMusicTransitionRuleFields,
    BankMusicTransitionSide, BankMusicTransitionSourceFields,
};
pub use p4kx_audio::{
    BankNodeAdvancedFields, BankNodeParameters, BankNodePlaybackFields, BankNodePositioningFields,
    BankPositioningModes, BankPositioningRow, BankPositioningRowFields, BankPositioningRows,
    BankPositioningTableKind, BankPositioningVector,
};
pub use p4kx_audio::{
    BankRandomSequenceFields, BankRandomSequenceItem, BankRandomSequencePlaylist,
    BankRandomSequenceRows,
};
pub use p4kx_audio::{
    BankSwitchConfiguration, BankSwitchConfigurationRows, BankSwitchContainerFields,
    BankSwitchExtent, BankSwitchGroup, BankSwitchGroupRows, BankSwitchMember, BankSwitchMembership,
};
use rayon::prelude::*;

#[path = "bank_read.rs"]
mod read;

use self::read::BankReadOrigin;

use crate::ConversionError;
use crate::output_file::{write_error, write_replacing_protected};
use crate::source_protection::SourceProtection;

/// The largest soundbank read into memory, far above the largest soundbank of a real Archive.
pub const MAX_SOUNDBANK_LEN: u64 = 512 << 20;

/// The file format [`Bank::extract_wems`] writes each embedded WEM in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WemFormat {
    /// The WEM bytes unchanged, as `<id>.wem`; a prefetched start is written as the bank holds it.
    Wem,
    /// A 16-bit PCM WAV, as `<id>.wav`; codecs without a decoder and prefetched starts fail per WEM.
    Wav,
    /// An Ogg Vorbis or Ogg Opus file rebuilt from a Wwise Vorbis or Opus WEM, as `<id>.ogg`;
    /// other codecs and prefetched starts fail per WEM.
    Ogg,
}

impl WemFormat {
    /// Returns the file extension, without the dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Wem => "wem",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
        }
    }
}

/// One embedded WEM [`Bank::extract_wems`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenWem {
    /// The WEM ID.
    pub id: u32,
    /// The file written.
    pub path: PathBuf,
    /// Its size in bytes.
    pub size: u64,
    /// Whether it is only the prefetched start of a streamed WEM, written as the bank holds it.
    pub prefetch: bool,
}

/// One embedded WEM [`Bank::extract_wems`] could not write.
#[derive(Debug)]
pub struct WemFailure {
    /// The WEM ID.
    pub id: u32,
    /// Why.
    pub error: ConversionError,
}

/// What [`Bank::extract_wems`] did, both lists in ascending ID order.
#[derive(Debug, Default)]
pub struct BankExtraction {
    /// The WEMs written.
    pub written: Vec<WrittenWem>,
    /// The WEMs that failed.
    pub failures: Vec<WemFailure>,
}

/// A Wwise BNK soundbank read from an Entry of an Archive or from a file on disk.
#[derive(Debug)]
pub struct Bank {
    protection: SourceProtection,
    name: String,
    soundbank: Soundbank<Vec<u8>>,
    origin: BankReadOrigin,
}

impl Bank {
    fn parse(
        name: String,
        bytes: Option<Vec<u8>>,
        protection: SourceProtection,
        origin: BankReadOrigin,
    ) -> Result<Self, ConversionError> {
        let Some(bytes) = bytes else {
            return Err(ConversionError::SoundTooLarge {
                sound: name,
                limit: MAX_SOUNDBANK_LEN,
            });
        };
        match Soundbank::parse(bytes) {
            Ok(soundbank) => Ok(Self {
                protection,
                name,
                soundbank,
                origin,
            }),
            Err(source) => Err(ConversionError::Soundbank { bank: name, source }),
        }
    }

    /// Returns the name of the soundbank: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parsed soundbank.
    #[must_use]
    pub const fn soundbank(&self) -> &Soundbank<Vec<u8>> {
        &self.soundbank
    }

    /// Writes every embedded WEM into `folder` as `<id>.<extension>` in parallel, replacing
    /// existing files; a WEM that fails leaves no file and does not stop the others.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::Write`] when `folder` cannot be created.
    pub fn extract_wems(
        &self,
        folder: &Path,
        format: WemFormat,
    ) -> Result<BankExtraction, ConversionError> {
        crate::check_protected_output(folder, self.protection.context()?, None)?;
        fs::create_dir_all(folder).map_err(|source| write_error(folder, source))?;
        let wems: Vec<EmbeddedWem<'_>> = self.soundbank.wems().collect();
        let outcomes: Vec<(u32, Result<WrittenWem, ConversionError>)> = wems
            .par_iter()
            .map(|wem| (wem.id(), self.write_wem(wem, folder, format)))
            .collect();
        let mut extraction = BankExtraction::default();
        for (id, outcome) in outcomes {
            match outcome {
                Ok(written) => extraction.written.push(written),
                Err(error) => extraction.failures.push(WemFailure { id, error }),
            }
        }
        Ok(extraction)
    }

    fn write_wem(
        &self,
        wem: &EmbeddedWem<'_>,
        folder: &Path,
        format: WemFormat,
    ) -> Result<WrittenWem, ConversionError> {
        if format != WemFormat::Wem && wem.is_prefetch() {
            return Err(ConversionError::PrefetchedWem {
                bank: self.name.clone(),
                id: wem.id(),
            });
        }
        let extension = if format == WemFormat::Wem && wem.class() == MediaClass::Midi {
            "mid"
        } else {
            format.extension()
        };
        let path = folder.join(format!("{}.{}", wem.id(), extension));
        let audio_error = |source| match source {
            AudioError::Write(source) => write_error(&path, source),
            source => ConversionError::Audio {
                sound: format!("{} WEM {}", self.name, wem.id()),
                source,
            },
        };
        let size =
            write_replacing_protected(
                &path,
                self.protection.context()?,
                None,
                |writer| match format {
                    WemFormat::Wem => {
                        writer
                            .write_all(wem.bytes())
                            .map_err(|source| write_error(&path, source))?;
                        Ok(wem.bytes().len() as u64)
                    }
                    WemFormat::Wav => wem
                        .parse()
                        .and_then(|parsed| parsed.write_wav(writer))
                        .map(|report| report.size)
                        .map_err(audio_error),
                    WemFormat::Ogg => wem
                        .parse()
                        .and_then(|parsed| parsed.write_ogg(writer))
                        .map(|report| report.size)
                        .map_err(audio_error),
                },
            )?;
        Ok(WrittenWem {
            id: wem.id(),
            path,
            size,
            prefetch: wem.is_prefetch(),
        })
    }
}

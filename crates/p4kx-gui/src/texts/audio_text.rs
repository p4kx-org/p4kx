// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// A text of the audio preview of a WEM, looked up in the Fluent file of the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioText {
    /// The preview tab that plays a sound.
    Tab,
    /// The key of the codec.
    Codec,
    /// The key of the number of channels.
    Channels,
    /// The key of the sample rate.
    SampleRate,
    /// The key of the playing time.
    Duration,
    /// A sample rate; takes `rate`.
    Hertz,
    /// Shown for a value the WEM does not state, such as an unknown playing time.
    Unknown,
    /// The action that plays the sound.
    Play,
    /// The action that pauses the sound.
    Pause,
    /// The accessible name of the position bar.
    Position,
    /// The label and accessible name of the volume bar.
    Volume,
    /// The position and the playing time; takes `position` and `duration`.
    Time,
    /// Shown while the sound is decoded; takes `path`.
    Decoding,
    /// Shown for a codec the preview cannot decode; takes `codec` and `path`.
    Unsupported,
    /// Shown when the decoded sound would be too large; takes `path` and `limit`.
    TooLong,
    /// Shown when the sound cannot be read or decoded; takes `path` and `reason`.
    Failed,
    /// Shown when the audio output fails; takes `reason`.
    OutputFailed,
    /// The key of the ID column of the WEMs of a soundbank.
    ColumnId,
    /// Shown instead of the codec of a WEM the soundbank holds only the prefetched start of.
    Prefetched,
    /// Shown instead of the controls for such a WEM; takes `id`.
    PrefetchedNote,
    /// Shown instead of the codec of media that is Wwise plug-in data.
    Plugin,
    /// Shown instead of the codec of media that is a MIDI file.
    Midi,
    /// Shown instead of the controls for such media; takes `id`.
    PluginNote,
    /// Shown instead of the controls for MIDI media; takes `id`.
    MidiNote,
    /// The path line of a soundbank: its size and the number of WEMs; takes `size`, `count` and
    /// `number`.
    BankSummary,
    /// The key of the count of embedded WEMs.
    BankWemCount,
    /// Shown for a soundbank that embeds no WEM; takes `path`.
    BankEmpty,
    /// Shown when a soundbank cannot be read; takes `path` and `reason`.
    BankFailed,
    /// The name of a WEM of a soundbank in the messages of the preview; takes `id` and `bank`.
    WemName,
    /// The accessible name of a row of the list; takes `id`, `codec` and `duration`.
    WemRow,
    /// Shown for a WEM of a soundbank with a codec the preview cannot decode; takes `codec` and
    /// `name`.
    UnsupportedWem,
}

impl AudioText {
    /// Every text of the audio preview.
    pub(crate) const ALL: [Self; 31] = [
        Self::Tab,
        Self::Codec,
        Self::Channels,
        Self::SampleRate,
        Self::Duration,
        Self::Hertz,
        Self::Unknown,
        Self::Play,
        Self::Pause,
        Self::Position,
        Self::Volume,
        Self::Time,
        Self::Decoding,
        Self::Unsupported,
        Self::TooLong,
        Self::Failed,
        Self::OutputFailed,
        Self::ColumnId,
        Self::Prefetched,
        Self::PrefetchedNote,
        Self::Plugin,
        Self::Midi,
        Self::PluginNote,
        Self::MidiNote,
        Self::BankSummary,
        Self::BankWemCount,
        Self::BankEmpty,
        Self::BankFailed,
        Self::WemName,
        Self::WemRow,
        Self::UnsupportedWem,
    ];

    /// Returns the Fluent message id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tab => "preview-tab-audio",
            Self::Codec => "audio-codec",
            Self::Channels => "audio-channels",
            Self::SampleRate => "audio-sample-rate",
            Self::Duration => "audio-duration",
            Self::Hertz => "audio-hertz",
            Self::Unknown => "audio-unknown",
            Self::Play => "audio-play",
            Self::Pause => "audio-pause",
            Self::Position => "audio-position",
            Self::Volume => "audio-volume",
            Self::Time => "audio-time",
            Self::Decoding => "audio-decoding",
            Self::Unsupported => "audio-unsupported",
            Self::TooLong => "audio-too-long",
            Self::Failed => "audio-failed",
            Self::OutputFailed => "audio-output-failed",
            Self::ColumnId => "audio-column-id",
            Self::Prefetched => "audio-prefetched",
            Self::PrefetchedNote => "audio-prefetched-note",
            Self::Plugin => "audio-plugin",
            Self::Midi => "audio-midi",
            Self::PluginNote => "audio-plugin-note",
            Self::MidiNote => "audio-midi-note",
            Self::BankSummary => "audio-bank-summary",
            Self::BankWemCount => "audio-bank-wem-count",
            Self::BankEmpty => "audio-bank-empty",
            Self::BankFailed => "audio-bank-failed",
            Self::WemName => "audio-wem-name",
            Self::WemRow => "audio-wem-row",
            Self::UnsupportedWem => "audio-unsupported-wem",
        }
    }
}

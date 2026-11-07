// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) mod exclude;
mod folder;
mod job;
mod summary;

use std::path::{Path, PathBuf};

use p4kx_extraction::{
    AudioConversion, ConflictPolicy, Exclusion, FilterError, TextureConversion,
    WriteProtectionError,
};
use p4kx_settings::{ExtractionAudio, ExtractionTextures};

use crate::ComparisonSide;
use crate::installation::refusal_of;

pub(crate) use folder::{FolderRequest, start_folder};
pub use job::{Extraction, ExtractionStatus};
pub(crate) use job::{Request, Source, start, target_naming};
pub use summary::{ExtractionSummary, FailedEntry, RecordCounts};

/// Which Entries an Extraction writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionScope {
    /// The checked Entries of the whole Archive.
    Selected,
    /// The Entries shown in the table.
    Shown,
    /// Every Entry of the Archive.
    All,
    /// The added and changed Entries of one side of the finished Comparison that its Filter
    /// selects and the exclusion leaves in; removed Entries never.
    Changes(ComparisonSide),
}

/// Where an Extraction writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TargetKind {
    /// A new folder `SC-EXPORT-<Channel>-<Build>-<timestamp>` created inside the chosen folder.
    #[default]
    NewFolder,
    /// The chosen folder itself, with a Conflict policy for files already there.
    ExistingFolder,
}

/// What a whole-Archive Extraction asks to confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirmation {
    /// The number of Entries, every Entry of the Archive.
    pub entries: usize,
    /// Their uncompressed size in bytes.
    pub bytes: u64,
}

/// The choices of an Extraction before it starts: scope, target, Conflict policy and Conversion.
#[derive(Debug, Clone)]
pub struct ExtractionSetup {
    scope: ExtractionScope,
    kind: TargetKind,
    folder: Option<PathBuf>,
    refusal: Option<WriteProtectionError>,
    policy: ConflictPolicy,
    convert: bool,
    textures: ExtractionTextures,
    audio: ExtractionAudio,
    confirmation: Option<Confirmation>,
    exclude: String,
    leaves_nothing: bool,
}

impl ExtractionSetup {
    pub(crate) fn new(
        scope: ExtractionScope,
        folder: Option<PathBuf>,
        convert: bool,
        textures: ExtractionTextures,
        audio: ExtractionAudio,
    ) -> Self {
        Self {
            scope,
            kind: TargetKind::NewFolder,
            refusal: refusal_of(folder.as_deref()),
            folder,
            policy: ConflictPolicy::Skip,
            convert,
            textures,
            audio,
            confirmation: None,
            exclude: String::new(),
            leaves_nothing: false,
        }
    }

    /// Returns which Entries are extracted.
    #[must_use]
    pub const fn scope(&self) -> ExtractionScope {
        self.scope
    }

    /// Chooses which Entries are extracted, withdrawing a pending confirmation.
    pub const fn set_scope(&mut self, scope: ExtractionScope) {
        self.scope = scope;
        self.confirmation = None;
        self.leaves_nothing = false;
    }

    /// Returns the text of the exclusion field: one Filter per line.
    #[must_use]
    pub fn exclude(&self) -> &str {
        &self.exclude
    }

    /// Replaces the exclusion text, withdrawing a pending confirmation.
    pub fn set_exclude(&mut self, text: String) {
        self.exclude = text;
        self.confirmation = None;
        self.leaves_nothing = false;
    }

    /// Returns the exclusion the text of the field stands for.
    ///
    /// # Errors
    ///
    /// Returns the [`FilterError`] of the first Filter that is not accepted.
    pub fn exclusion(&self) -> Result<Exclusion, FilterError> {
        exclude::parse(&self.exclude)
    }

    /// Returns whether the last attempt to start found that the exclusion leaves no Entry.
    #[must_use]
    pub const fn leaves_nothing(&self) -> bool {
        self.leaves_nothing
    }

    pub(crate) const fn set_leaves_nothing(&mut self) {
        self.leaves_nothing = true;
    }

    /// Returns where the Extraction writes.
    #[must_use]
    pub const fn kind(&self) -> TargetKind {
        self.kind
    }

    /// Chooses where the Extraction writes.
    pub const fn set_kind(&mut self, kind: TargetKind) {
        self.kind = kind;
    }

    /// Returns the chosen folder, or `None` while none is chosen.
    #[must_use]
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Chooses the folder the new folder is created in, or the existing folder written into.
    pub fn set_folder(&mut self, folder: PathBuf) {
        self.refusal = refusal_of(Some(&folder));
        self.folder = Some(folder);
    }

    /// Returns why the chosen folder cannot be written to, when it lies inside a Star Citizen
    /// installation.
    #[must_use]
    pub const fn refusal(&self) -> Option<&WriteProtectionError> {
        self.refusal.as_ref()
    }

    /// Returns the Conflict policy for files already in an existing folder.
    #[must_use]
    pub const fn policy(&self) -> ConflictPolicy {
        self.policy
    }

    /// Chooses the Conflict policy for files already in an existing folder.
    pub const fn set_policy(&mut self, policy: ConflictPolicy) {
        self.policy = policy;
    }

    /// Returns whether `CryXml` Entries and textures are converted.
    #[must_use]
    pub const fn convert(&self) -> bool {
        self.convert
    }

    /// Switches the Conversion of `CryXml` Entries and textures on or off.
    pub const fn set_convert(&mut self, convert: bool) {
        self.convert = convert;
    }

    /// Returns what a Conversion writes for a texture.
    #[must_use]
    pub const fn textures(&self) -> ExtractionTextures {
        self.textures
    }

    /// Chooses what a Conversion writes for a texture.
    pub const fn set_textures(&mut self, textures: ExtractionTextures) {
        self.textures = textures;
    }

    /// Returns what a Conversion writes for a sound and for each WEM a soundbank embeds.
    #[must_use]
    pub const fn audio(&self) -> ExtractionAudio {
        self.audio
    }

    /// Chooses what a Conversion writes for a sound and for each WEM a soundbank embeds.
    pub const fn set_audio(&mut self, audio: ExtractionAudio) {
        self.audio = audio;
    }

    /// Returns whether a whole-Archive Extraction waits for confirmation.
    #[must_use]
    pub const fn is_confirming(&self) -> bool {
        self.confirmation.is_some()
    }

    /// Returns what a whole-Archive Extraction waits to confirm, or `None` when it does not.
    #[must_use]
    pub const fn confirmation(&self) -> Option<Confirmation> {
        self.confirmation
    }

    /// Withdraws a pending confirmation, back to the choices.
    pub const fn withdraw_confirmation(&mut self) {
        self.confirmation = None;
    }

    pub(crate) const fn ask_confirmation(&mut self, confirmation: Confirmation) {
        self.confirmation = Some(confirmation);
    }

    pub(crate) fn request(&self, source: Source, excluded_parts: usize) -> Option<Request> {
        if self.refusal.is_some() {
            return None;
        }
        Some(Request {
            source,
            kind: self.kind,
            folder: self.folder.clone()?,
            policy: match self.kind {
                TargetKind::NewFolder => ConflictPolicy::Skip,
                TargetKind::ExistingFolder => self.policy,
            },
            convert: self.convert,
            textures: self.textures,
            audio: self.audio,
            excluded_parts,
        })
    }
}

/// Returns the sound option of the Extraction service for the stored choice.
pub(crate) const fn audio_conversion(audio: ExtractionAudio) -> AudioConversion {
    match audio {
        ExtractionAudio::Ogg => AudioConversion::Ogg,
        ExtractionAudio::Wav => AudioConversion::Wav,
        ExtractionAudio::Raw => AudioConversion::Raw,
    }
}

/// Returns the texture options of the Extraction service for the stored choices.
pub(crate) const fn texture_conversion(textures: ExtractionTextures) -> TextureConversion {
    TextureConversion {
        merge: textures.merge,
        png: if textures.png {
            Some(textures.mip)
        } else {
            None
        },
        keep_raw: textures.keep_raw,
    }
}

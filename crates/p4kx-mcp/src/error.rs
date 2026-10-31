// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::PathBuf;

use p4kx_assembly::{ChangeError, EntityLookupError, LoadoutError};
use p4kx_extraction::model::MaterialError;
use p4kx_extraction::{ArchiveError, ConversionError, DataForgeError, EntryReadError, FilterError};
use p4kx_settings::{KeyChoiceError, SettingsError};
use thiserror::Error;

/// A tool call that cannot be answered; its message goes back to the client as a tool error.
#[derive(Debug, Error)]
pub(crate) enum ToolError {
    /// Neither the call nor the server argument names an Archive.
    #[error("no Archive given: pass `archive` or start the server with `--archive`")]
    NoArchive,
    /// The text names no detected Channel.
    #[error("`{name}` is no detected Channel; detected: {detected}")]
    UnknownChannel {
        /// The Channel name asked for.
        name: String,
        /// The detected Channel names, comma separated.
        detected: String,
    },
    /// The text is no absolute local path of a `.p4k` file, or of a `.dcb` file for the DataForge
    /// tools.
    #[error(
        "`{0}` is neither a Channel name nor the absolute local path of a `.p4k` file (the \
         DataForge tools also take a `.dcb` file)"
    )]
    NotAnArchivePath(String),
    /// The path names no existing file.
    #[error("{} does not exist or is not a file", .0.display())]
    Missing(PathBuf),
    /// The path resolves to a network drive, a device or a file of another kind.
    #[error(
        "{} does not resolve to a `.p4k` file on a local drive (the DataForge tools also take \
         a `.dcb` file)",
        .0.display()
    )]
    NotLocal(PathBuf),
    /// An argument exceeds its size cap.
    #[error("`{name}` is longer than {max} bytes")]
    TooLong {
        /// The argument name.
        name: &'static str,
        /// The cap in bytes.
        max: usize,
    },
    /// A Filter is too long or has too many wildcards.
    #[error("the Filter is not accepted")]
    Filter(#[from] FilterError),
    /// The cursor is not one this tool returned for this listing.
    #[error("the cursor `{0}` is not one this listing returned")]
    BadCursor(String),
    /// The Archive cannot be opened.
    #[error("cannot open the Archive {}", .path.display())]
    Open {
        /// The Archive path.
        path: PathBuf,
        /// What failed.
        #[source]
        source: ArchiveError,
    },
    /// The Archive holds no Entry with the path.
    #[error("the Archive holds no Entry `{0}`; search_entries finds Entries by path")]
    UnknownEntry(String),
    /// A container path has an empty container name.
    #[error(
        "`{0}` is no container path; write it as outer.socpak::inner.socpak::path with every container named"
    )]
    BadContainerPath(String),
    /// An Entry cannot be opened as a container.
    #[error("cannot open the container {path}")]
    Container {
        /// The container's path, with the containers it lies in.
        path: String,
        /// What failed.
        #[source]
        source: ArchiveError,
    },
    /// The request asks for something the tool does not give; the message says what and how to
    /// ask for less.
    #[error("{0}")]
    Refused(String),
    /// A material library cannot be read.
    #[error("cannot read the material library {path}")]
    Material {
        /// The Entry path.
        path: String,
        /// What failed.
        #[source]
        source: MaterialError,
    },
    /// No Entity is named by the text.
    #[error("cannot find the Entity")]
    Entity(#[from] EntityLookupError),
    /// The Loadout of the Entity cannot be resolved at all.
    #[error("cannot resolve the Loadout")]
    Loadout(#[from] LoadoutError),
    /// The Loadout picks of the call are no picks.
    #[error("the Loadout picks are not valid")]
    Picks(#[source] ChangeError),
    /// The Entry holds binary data where text was asked for.
    #[error("the Entry {0} holds binary data, not text; read it with read_entry_bytes")]
    Binary(String),
    /// The Entry is too large to decode as text.
    #[error(
        "the Entry {path} is too large to read as text: more than {limit} bytes to decode; read it with read_entry_bytes"
    )]
    TooLarge {
        /// The Entry path.
        path: String,
        /// The limit in bytes.
        limit: u64,
    },
    /// The Entry content cannot be read.
    #[error("cannot read the Entry {path}")]
    Read {
        /// The Entry path.
        path: String,
        /// What failed.
        #[source]
        source: EntryReadError,
    },
    /// The settings file with the manual Channel entries cannot be read.
    #[error("cannot read the settings")]
    Settings(#[from] SettingsError),
    /// A source of the Archive key holds no valid key, e.g. the stored key file is malformed.
    #[error("the Archive key cannot be used")]
    Key(#[from] KeyChoiceError),
    /// The worker running the call ended without a result.
    #[error("the tool call stopped unexpectedly")]
    Stopped,
    /// An error several calls received together, such as a container they waited for.
    #[error(transparent)]
    Shared(std::sync::Arc<Self>),
    /// The DataForge of the Archive cannot be read; the message holds every cause.
    #[error("cannot read the DataForge: {0}")]
    NoDataForge(String),
    /// A value of the DataForge cannot be read.
    #[error("the DataForge cannot be read")]
    DataForge(#[from] DataForgeError),
    /// The call names neither a GUID nor a record path, or both.
    #[error("pass either `guid` or `path`")]
    RecordSelector,
    /// The text is no GUID.
    #[error("`{0}` is no GUID; write it as 01234567-89ab-cdef-0123-456789abcdef")]
    BadGuid(String),
    /// No Record has the GUID.
    #[error("no Record has the GUID {0}; search_records finds Records")]
    UnknownGuid(String),
    /// No Record has the record path.
    #[error("no Record has the record path `{0}`; search_records finds Records")]
    UnknownRecordPath(String),
    /// The Entry is no chunk file or no model, or one of its parts cannot be read.
    #[error("cannot read the Entry {path} as {what}")]
    Convert {
        /// The Entry path.
        path: String,
        /// What it was read as, e.g. `a chunk file`.
        what: &'static str,
        /// What failed.
        #[source]
        source: Box<ConversionError>,
    },
    /// The chunk type filter is neither a known type name nor a type ID.
    #[error(
        "`{0}` is neither a chunk type name known for this file's container nor a hexadecimal type ID such as 0x9293B9D8; list_chunks without `type` shows the types the file holds"
    )]
    BadChunkType(String),
    /// One item alone is larger than a response may be.
    #[error("the {what} is larger than the response cap of {limit} bytes; {hint}")]
    TooLargeItem {
        /// What is too large, e.g. `Record EntityClassDefinition.AEGS_Avenger`.
        what: String,
        /// The cap in bytes.
        limit: usize,
        /// How to ask for less.
        hint: &'static str,
    },
    /// The Query selects more columns or compares more values than a call may.
    #[error("the Query has {count} {what}, more than the {max} a call may have; split it")]
    QueryTooComplex {
        /// What there is too much of.
        what: &'static str,
        /// How many there are.
        count: usize,
        /// The most there may be.
        max: usize,
    },
    /// Selecting the Records of the Query took too long or the call was cancelled.
    #[error(
        "selecting the Records was stopped after at most {0} seconds or by the client; narrow the Query with `from`, `at` or `where`"
    )]
    QueryStopped(u64),
    /// No ATL definition has the audio trigger.
    #[error("no ATL definition has the audio trigger `{0}`; search_audio_triggers finds triggers")]
    UnknownTrigger(String),
    /// The audio language is no language folder of the Archive.
    #[error(
        "the Archive holds no audio language `{language}`; its language folders are: {available}"
    )]
    UnknownLanguage {
        /// The language asked for.
        language: String,
        /// The language folders of the Archive, comma separated.
        available: String,
    },
    /// The call names neither a WEM ID nor an Entry path, or both.
    #[error("pass either `wem_id` or `path`")]
    WemSelector,
    /// No soundbank embeds the WEM and no loose WEM Entry has its ID.
    #[error("no soundbank embeds the WEM {0} and no loose WEM Entry has that ID")]
    UnknownWem(u32),
    /// Only the prefetched start of the WEM is in the Archive.
    #[error("the Archive holds only the prefetched start of the WEM {0}, which cannot be decoded")]
    PrefetchOnly(u32),
    /// The soundbank Entry is larger than the audio tools read.
    #[error("the soundbank {path} is larger than the {limit} bytes the audio tools read")]
    SoundbankTooLarge {
        /// The Entry path.
        path: String,
        /// The cap in bytes.
        limit: u64,
    },
    /// The sound has no frames.
    #[error("the sound has no audio frames")]
    EmptySound,
    /// The excerpt lies beyond the part of a sound that is decoded for one.
    #[error(
        "the excerpt lies beyond the first {limit} bytes of decoded audio, too far in for an excerpt"
    )]
    ExcerptSource {
        /// The decoded size cap in bytes.
        limit: u64,
    },
    /// The excerpt starts behind the end of the sound.
    #[error("the sound is only {seconds:.3} seconds long; start_seconds is behind its end")]
    ExcerptStart {
        /// The playing time of the sound.
        seconds: f64,
    },
    /// The excerpt window is not a finite, non-negative number of seconds.
    #[error("`{0}` must be a finite number of seconds, not negative")]
    BadSeconds(&'static str),
    /// A result cannot be converted to JSON.
    #[error("cannot convert the result to JSON")]
    Json(#[from] serde_json::Error),
    /// A result is not valid UTF-8.
    #[error("the result is not valid UTF-8")]
    Utf8(#[from] std::string::FromUtf8Error),
    /// A result cannot be written.
    #[error("cannot write the result")]
    Write(#[from] std::io::Error),
    /// The Query cannot be parsed or compiled; the message marks the span.
    #[error("the Query is invalid at bytes {start}..{end}: {marked}")]
    Query {
        /// The offset of the first byte of the span.
        start: usize,
        /// The offset behind the span.
        end: usize,
        /// The message with the marked line of the Query.
        marked: String,
    },
}

impl From<ChangeError> for ToolError {
    /// A Loadout that cannot be resolved stays [`ToolError::Loadout`]; any other change error
    /// comes from the picks.
    fn from(error: ChangeError) -> Self {
        match error {
            ChangeError::Loadout(error) => Self::Loadout(error),
            other => Self::Picks(other),
        }
    }
}

impl ToolError {
    /// Returns the error of `path` that could not be converted `what`: an unknown Entry, or the
    /// conversion error.
    pub(crate) fn converted(path: &str, what: &'static str, error: ConversionError) -> Self {
        if matches!(error, ConversionError::UnknownEntry(_)) {
            Self::UnknownEntry(path.to_owned())
        } else {
            Self::Convert {
                path: path.to_owned(),
                what,
                source: Box::new(error),
            }
        }
    }

    /// Returns the message with every cause, `: ` separated.
    pub(crate) fn message(&self) -> String {
        let mut message = chain(self);
        if let Some(hint) = crate::keys::hint(self) {
            message.push_str(". ");
            message.push_str(hint);
        }
        message
    }
}

/// Returns the message of `error` with every cause, `: ` separated.
pub(crate) fn chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// Returns `text` when it fits `max` bytes.
pub(crate) const fn capped<'a>(
    name: &'static str,
    text: &'a str,
    max: usize,
) -> Result<&'a str, ToolError> {
    if text.len() > max {
        Err(ToolError::TooLong { name, max })
    } else {
        Ok(text)
    }
}

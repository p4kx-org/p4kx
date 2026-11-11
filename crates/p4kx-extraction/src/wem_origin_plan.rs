// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{Codec, WemCodecSource, WemInfoSource};
use thiserror::Error;

use crate::SourceUseRole;

#[path = "wem_origin_count.rs"]
mod count;
#[path = "wem_origin_loop.rs"]
mod loops;
#[path = "wem_origin_site.rs"]
mod site;
pub use site::WemOriginSite;

/// A WEM summary value whose evidence is selected by the original parser decisions.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WemOriginField {
    /// Detected codec or its display name.
    Codec,
    /// Raw format tag.
    FormatTag,
    /// Declared number of channels.
    Channels,
    /// Effective playback sample rate.
    SampleRate,
    /// Frame count or its absence.
    SampleCount,
    /// Playback duration or its absence.
    Duration,
    /// Selected loop or its absence.
    Loop,
    /// Start frame of a selected loop.
    LoopStart,
    /// Exclusive end frame of a selected loop.
    LoopEnd,
    /// Length in frames of a selected loop.
    LoopFrames,
    /// Byte order declared by the RIFF signature.
    RiffByteOrder,
    /// A declaration from the selected fmt body.
    Format(WemFormatField),
}

/// Additional fields of the selected WEM format.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum WemFormatField {
    /// Sample rate declared in fmt.
    DeclaredSampleRate,
    /// Alignment in bytes per encoded block.
    BlockAlign,
    /// Bits per sample declared in fmt.
    BitsPerSample,
    /// Original Wwise channel layout word or its absence.
    ChannelLayout {
        /// The selected body contains the complete layout word.
        present: bool,
    },
    /// Speaker mask derived from the original channel layout.
    ChannelMask {
        /// The selected body contains the complete layout word.
        layout_present: bool,
    },
}

/// WEM evidence could not be mapped to the requested output value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum WemOriginError {
    /// The parser reported a decision this planner does not cover.
    #[error("unhandled original WEM decision")]
    UnhandledDecision,
    /// A loop member was requested without a retained loop.
    #[error("selected WEM loop member has no original entry")]
    MissingLoop,
}

/// Maps WEM parser decisions to the original inputs that contributed to each output value.
#[derive(Debug)]
pub struct WemOriginPlan<'info> {
    info: &'info WemInfoSource,
    codec: Codec,
    sample_count: Option<u64>,
}

impl<'info> WemOriginPlan<'info> {
    /// Uses the decisions and summary values from the same WEM parse.
    #[must_use]
    pub const fn new(info: &'info WemInfoSource, codec: Codec, sample_count: Option<u64>) -> Self {
        Self {
            info,
            codec,
            sample_count,
        }
    }

    /// Returns the original contributions for one datum in its recorded parse.
    ///
    /// # Errors
    /// Returns an error for an unsupported decision or an unselected loop member.
    pub fn field(&self, field: WemOriginField) -> Result<Vec<WemOriginSite>, WemOriginError> {
        use SourceUseRole::{Conversion, Payload};

        match field {
            WemOriginField::Codec => self.codec(Conversion),
            WemOriginField::FormatTag => Ok(self.direct_format("formatTag")),
            WemOriginField::Channels => Ok(self.direct_format("channels")),
            WemOriginField::SampleRate => self.rate(Payload),
            WemOriginField::SampleCount => count::inputs(self, Payload),
            WemOriginField::Duration => {
                let mut inputs = count::inputs(self, Conversion)?;
                if self.sample_count.is_some() {
                    inputs.extend(self.rate(Conversion)?);
                }
                Ok(inputs)
            }
            WemOriginField::Loop
            | WemOriginField::LoopStart
            | WemOriginField::LoopEnd
            | WemOriginField::LoopFrames => loops::inputs(self, field),
            WemOriginField::RiffByteOrder => Ok(vec![Self::signature()]),
            WemOriginField::Format(format) => Ok(self.format_field(format)),
        }
    }

    fn selected(&self) -> Vec<WemOriginSite> {
        vec![self.routing(), Self::signature()]
    }

    const fn signature() -> WemOriginSite {
        WemOriginSite::global("wemRiffHeader", "signature", SourceUseRole::Conversion)
    }

    const fn routing(&self) -> WemOriginSite {
        WemOriginSite::single(
            "wemRiffPrefix",
            self.info.data_body_offset(),
            "chunkRouting",
            SourceUseRole::Selector,
        )
    }

    fn direct_format(&self, name: &'static str) -> Vec<WemOriginSite> {
        let mut inputs = self.selected();
        inputs.push(self.format(name, SourceUseRole::Payload));
        inputs
    }

    fn format_field(&self, field: WemFormatField) -> Vec<WemOriginSite> {
        match field {
            WemFormatField::DeclaredSampleRate => self.direct_format("sampleRate"),
            WemFormatField::BlockAlign => self.direct_format("blockAlign"),
            WemFormatField::BitsPerSample => self.direct_format("bitsPerSample"),
            WemFormatField::ChannelLayout { present } => {
                self.layout(present, SourceUseRole::Payload)
            }
            WemFormatField::ChannelMask { layout_present } => {
                let mut inputs = self.layout(layout_present, SourceUseRole::Conversion);
                if layout_present {
                    inputs.push(self.format("channels", SourceUseRole::Selector));
                }
                inputs
            }
        }
    }

    fn layout(&self, present: bool, role: SourceUseRole) -> Vec<WemOriginSite> {
        let mut inputs = self.selected();
        inputs.push(self.format("bodyLength", SourceUseRole::Selector));
        if present {
            inputs.push(self.format("channelLayout", role));
        }
        inputs
    }

    fn codec(&self, role: SourceUseRole) -> Result<Vec<WemOriginSite>, WemOriginError> {
        let mut inputs = self.selected();
        inputs.push(self.format("formatTag", role));
        let layout = match self.info.codec() {
            WemCodecSource::Tag => return Ok(inputs),
            WemCodecSource::DspCoefficients => "extraLength",
            WemCodecSource::AdpcmBlock {
                extra_length_present,
            } => {
                inputs.push(self.format("blockAlign", role));
                if extra_length_present {
                    "extraLength"
                } else {
                    "bodyLength"
                }
            }
            _ => return Err(WemOriginError::UnhandledDecision),
        };
        inputs.push(self.format("channels", role));
        inputs.push(self.format(layout, SourceUseRole::Selector));
        Ok(inputs)
    }

    fn rate(&self, role: SourceUseRole) -> Result<Vec<WemOriginSite>, WemOriginError> {
        let mut inputs = self.codec(SourceUseRole::Selector)?;
        if self.codec != Codec::Opus {
            inputs.push(self.format("sampleRate", role));
        }
        Ok(inputs)
    }

    const fn format(&self, name: &'static str, role: SourceUseRole) -> WemOriginSite {
        Self::chunk(self.info.format_body_offset(), name, role)
    }

    const fn chunk(body_offset: u64, name: &'static str, role: SourceUseRole) -> WemOriginSite {
        WemOriginSite::single("wemChunk", body_offset, name, role)
    }
}

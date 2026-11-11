// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::panic::{self, AssertUnwindSafe};

use opus_pure::OpusDecoder;

use super::framing::{self, MAX_PACKET_SAMPLES};
use super::header::StreamHeader;
use super::layout::{Layout, SILENT};
use super::stream::{PacketCount, PacketSource, WriteError};
use super::{OPUS_RATE, OpusError};
use crate::{AudioError, WavReport, Wem};

const PANICKED: &str = "the decoder failed";
const MAX_SAMPLES_PER_CHANNEL: usize = MAX_PACKET_SAMPLES as usize;

impl<B: AsRef<[u8]>> Wem<B> {
    /// Decodes an Opus sound and writes it as a 16-bit PCM WAV to `out`.
    pub(crate) fn write_opus_wav(&self, out: &mut impl Write) -> Result<WavReport, AudioError> {
        let stream = self.opus_stream()?;
        let (header, report) = self.decoded_wav_header(stream.frames())?;
        out.write_all(&header).map_err(AudioError::Write)?;
        let mut packets = stream.packets().map_err(AudioError::InvalidOpus)?;
        write(&stream.header, &stream.count, &mut packets, out).map_err(WriteError::into_audio)?;
        Ok(report)
    }
}

/// Decodes the selected packets to interleaved little-endian 16-bit frames in the WEM's order.
pub(super) fn write<S: PacketSource>(
    header: &StreamHeader,
    count: &PacketCount,
    packets: &mut S,
    out: &mut (impl Write + ?Sized),
) -> Result<(), WriteError<S::Error>> {
    let invalid = |cause: OpusError| WriteError::Input(cause.into());
    let mut decoders = Decoders::new(&header.layout).map_err(invalid)?;
    let width = header.layout.decoded_channels();
    let mut skip = usize::from(count.pre_skip);
    let mut remaining = count.frames;
    let mut bytes = Vec::new();
    let gain = Gain::new(header.output_gain);
    let mut index = 0;
    while remaining > 0 {
        let Some(packet) = packets.read_packet().map_err(WriteError::Input)? else {
            break;
        };
        let decoded = decoders.decode(packet, index).map_err(invalid)?;
        index += 1;
        let frames = decoded.len() / width;
        let skipped = skip.min(frames);
        skip -= skipped;
        let take =
            usize::try_from(remaining).map_or(frames - skipped, |left| left.min(frames - skipped));
        bytes.clear();
        for frame in decoded.chunks_exact(width).skip(skipped).take(take) {
            for &channel in &header.layout.wav_order {
                let sample = if channel == SILENT {
                    0
                } else {
                    frame.get(usize::from(channel)).copied().unwrap_or_default()
                };
                bytes.extend_from_slice(&gain.apply(sample).to_le_bytes());
            }
        }
        out.write_all(&bytes).map_err(WriteError::Output)?;
        remaining -= take as u64;
    }
    if remaining > 0 {
        return Err(invalid(OpusError::MissingSamples {
            expected: count.frames,
            decoded: count.frames - remaining,
        }));
    }
    Ok(())
}

/// The output gain of an `OpusHead`, applied to every decoded sample.
struct Gain(Option<f32>);

impl Gain {
    fn new(q8_db: i16) -> Self {
        Self((q8_db != 0).then(|| 10_f32.powf(f32::from(q8_db) / (256.0 * 20.0))))
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the product is clamped to the i16 range first"
    )]
    fn apply(&self, sample: i16) -> i16 {
        self.0.map_or(sample, |factor| {
            (f32::from(sample) * factor)
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
        })
    }
}

/// One Opus decoder per stream of a packet, and the buffers they decode into.
struct Decoders {
    streams: Vec<OpusDecoder>,
    coupled: usize,
    width: usize,
    stream_pcm: Vec<i16>,
    pcm: Vec<i16>,
    packet: Vec<u8>,
}

impl Decoders {
    fn new(layout: &Layout) -> Result<Self, OpusError> {
        let coupled = usize::from(layout.coupled);
        let decoders = (0..usize::from(layout.streams))
            .map(|stream| {
                let channels = if stream < coupled { 2 } else { 1 };
                OpusDecoder::new(OPUS_RATE.cast_signed(), channels)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| OpusError::Undecodable {
                index: 0,
                reason: error.to_string(),
            })?;
        let width = layout.decoded_channels();
        Ok(Self {
            streams: decoders,
            coupled,
            width,
            stream_pcm: vec![0; MAX_SAMPLES_PER_CHANNEL * 2],
            pcm: vec![0; MAX_SAMPLES_PER_CHANNEL * width],
            packet: Vec::new(),
        })
    }

    /// Decodes a packet and returns its frames, interleaved in the order of the streams.
    fn decode(&mut self, packet: &[u8], index: u64) -> Result<&[i16], OpusError> {
        let Self {
            streams: decoders,
            coupled,
            width,
            stream_pcm,
            pcm,
            packet: scratch,
        } = self;
        let streams = decoders.len();
        let mut rest = packet;
        let mut frames = None;
        let mut channel = 0;
        for (stream, decoder) in decoders.iter_mut().enumerate() {
            let data = if stream + 1 == streams {
                rest
            } else {
                let sub = framing::self_delimited(rest)
                    .map_err(|reason| OpusError::InvalidPacket { index, reason })?;
                scratch.clear();
                sub.normal_into(rest, scratch);
                rest = rest.get(sub.end..).unwrap_or_default();
                scratch.as_slice()
            };
            let undecodable = |reason: String| OpusError::Undecodable { index, reason };
            let decoded = panic::catch_unwind(AssertUnwindSafe(|| {
                decoder
                    .decode_s16(data, MAX_SAMPLES_PER_CHANNEL, stream_pcm)
                    .map_err(|error| error.to_string())
            }))
            .unwrap_or_else(|_| Err(PANICKED.to_owned()))
            .map_err(undecodable)?;
            if *frames.get_or_insert(decoded) != decoded {
                return Err(undecodable("the streams differ in duration".to_owned()));
            }
            let channels = if stream < *coupled { 2 } else { 1 };
            let end = channel + channels;
            for (target, source) in pcm
                .chunks_exact_mut(*width)
                .zip(stream_pcm.chunks_exact(channels))
                .take(decoded)
            {
                if let Some(target) = target.get_mut(channel..end) {
                    target.copy_from_slice(source);
                }
            }
            channel = end;
        }
        let len = frames.unwrap_or_default() * *width;
        Ok(pcm.get(..len).unwrap_or_default())
    }
}

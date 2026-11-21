// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Encoders for the Wwise ADPCM codecs: every sub-block starts at the step index that encodes it
//! best, and each nibble is the one whose decoded sample lies closest to the input.

const IMA_SUB_BLOCK_LEN: usize = 0x24;
const IMA_HEADER_LEN: usize = 4;
const IMA_MAX_INDEX: u8 = 88;
const PT_HEADER_LEN: usize = 5;
const PT_MAX_INDEX: u8 = 11;

const IMA_STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const IMA_INDEX_STEPS: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

const PT_FIRST_ROW: [(i32, u8); 8] = [
    (14, 2),
    (10, 2),
    (7, 1),
    (5, 1),
    (3, 0),
    (2, 0),
    (1, 0),
    (0, 0),
];
const PT_MAGNITUDES: [i32; 8] = [28, 20, 14, 10, 7, 5, 3, 1];
const PT_INDEX_STEPS: [u8; 8] = [2, 2, 1, 1, 0, 0, 0, 0];

/// One channel's encoded sub-block or frame and its summed absolute error.
struct Encoded {
    bytes: Vec<u8>,
    error: i64,
}

/// Encodes interleaved samples as Wwise IMA ADPCM: blocks of one 36-byte sub-block per channel,
/// 64 frames each. The last block is padded with silence.
pub(crate) fn encode_ima(samples: &[i16], channels: u16) -> Vec<u8> {
    let frames = (IMA_SUB_BLOCK_LEN - IMA_HEADER_LEN) * 2;
    encode(samples, channels, frames, |input| {
        best((0..=IMA_MAX_INDEX).map(|index| ima_sub_block(input, index)))
    })
}

/// Encodes interleaved samples as PTADPCM: blocks of one `frame_len`-byte frame per channel. The
/// last block is padded with silence.
pub(crate) fn encode_ptadpcm(samples: &[i16], channels: u16, frame_len: usize) -> Vec<u8> {
    let frames = 2 + (frame_len.max(PT_HEADER_LEN + 1) - PT_HEADER_LEN) * 2;
    encode(samples, channels, frames, |input| {
        best((0..=PT_MAX_INDEX).map(|index| pt_frame(input, index)))
    })
}

fn encode(
    samples: &[i16],
    channels: u16,
    frames: usize,
    sub_block: impl Fn(&[i32]) -> Vec<u8>,
) -> Vec<u8> {
    let channels = usize::from(channels.max(1));
    let mut out = Vec::new();
    for block in samples.chunks(frames * channels) {
        for channel in 0..channels {
            let mut input: Vec<i32> = block
                .iter()
                .skip(channel)
                .step_by(channels)
                .map(|&sample| i32::from(sample))
                .collect();
            input.resize(frames, 0);
            out.extend(sub_block(&input));
        }
    }
    out
}

fn best(candidates: impl Iterator<Item = Encoded>) -> Vec<u8> {
    candidates
        .min_by_key(|encoded| encoded.error)
        .map_or_default(|encoded| encoded.bytes)
}

fn ima_sub_block(input: &[i32], start: u8) -> Encoded {
    let mut history = input.first().copied().unwrap_or(0);
    let mut index = i32::from(start);
    let mut bytes = clamp16(history).to_le_bytes().to_vec();
    bytes.extend([start, 0]);
    let mut nibbles = Vec::new();
    let mut error = 0;
    for &target in input.iter().skip(1) {
        let (nibble, sample, next) = closest(target, |nibble| {
            let (sample, next) = ima_expand(history, index, nibble);
            (sample, next, 0)
        });
        error += i64::from((sample - target).abs());
        nibbles.push(nibble);
        history = sample;
        index = next;
    }
    nibbles.push(0);
    bytes.extend(pack(&nibbles));
    Encoded { bytes, error }
}

fn ima_expand(history: i32, index: i32, nibble: u8) -> (i32, i32) {
    let step = usize::try_from(index)
        .ok()
        .and_then(|index| IMA_STEPS.get(index))
        .copied()
        .unwrap_or(0);
    let delta = ((i32::from(nibble & 7) * 2 + 1) * step) >> 3;
    let sample = clamp(history + if nibble & 8 == 0 { delta } else { -delta });
    let index_step = IMA_INDEX_STEPS
        .get(usize::from(nibble & 7))
        .copied()
        .unwrap_or(0);
    (
        sample,
        (index + index_step).clamp(0, i32::from(IMA_MAX_INDEX)),
    )
}

fn pt_frame(input: &[i32], start: u8) -> Encoded {
    let mut older = input.first().copied().unwrap_or(0);
    let mut newer = input.get(1).copied().unwrap_or(0);
    let mut index = start;
    let mut bytes = clamp16(older).to_le_bytes().to_vec();
    bytes.extend(clamp16(newer).to_le_bytes());
    bytes.push(start);
    let mut nibbles = Vec::new();
    let mut error = 0;
    for (position, &target) in input.iter().skip(2).enumerate() {
        let ahead = input.get(position + 3).copied();
        let (nibble, sample, next) = closest(target, |nibble| {
            let (step, next) = pt_step(index, nibble);
            let sample = clamp(step + 2 * newer - older);
            let lookahead = ahead.map_or(0, |ahead| {
                let (_, candidate, _) = closest(ahead, |nibble| {
                    let (step, after) = pt_step(next, nibble);
                    (clamp(step + 2 * sample - newer), after, 0)
                });
                (candidate - ahead).abs()
            });
            (sample, next, lookahead)
        });
        error += i64::from((sample - target).abs());
        nibbles.push(nibble);
        older = newer;
        newer = sample;
        index = next;
    }
    bytes.extend(pack(&nibbles));
    Encoded { bytes, error }
}

/// Returns the step and the next index of the Platinum table from its pattern: from index 1 on,
/// every row doubles the one before, and nibbles `k` and `15 − k` are negatives of each other.
fn pt_step(index: u8, nibble: u8) -> (i32, u8) {
    let k = usize::from(if nibble < 8 { nibble } else { 15 - nibble });
    let (magnitude, next) = if index == 0 {
        PT_FIRST_ROW.get(k).copied().unwrap_or((0, 0))
    } else {
        let magnitude = PT_MAGNITUDES.get(k).copied().unwrap_or(0) << (index - 1);
        let next = if k == 7 {
            index - 1
        } else {
            index + PT_INDEX_STEPS.get(k).copied().unwrap_or(0)
        };
        (magnitude, next.min(PT_MAX_INDEX))
    };
    (if nibble < 8 { -magnitude } else { magnitude }, next)
}

/// Returns the nibble whose decoded sample, plus the extra cost the decoder reports, lies closest
/// to `target`, with that sample and the next decoder state.
fn closest<I: Copy>(target: i32, decode: impl Fn(u8) -> (i32, I, i32)) -> (u8, i32, I) {
    let cost = |(sample, _, extra): (i32, I, i32)| (sample - target).abs() + extra;
    let first = decode(0);
    let (nibble, (sample, next, _)) = (1..16).fold((0, first), |chosen, nibble| {
        let candidate = decode(nibble);
        if cost(candidate) < cost(chosen.1) {
            (nibble, candidate)
        } else {
            chosen
        }
    });
    (nibble, sample, next)
}

fn pack(nibbles: &[u8]) -> impl Iterator<Item = u8> {
    nibbles
        .chunks(2)
        .map(|pair| pair.first().copied().unwrap_or(0) | (pair.get(1).copied().unwrap_or(0) << 4))
}

fn clamp(sample: i32) -> i32 {
    sample.clamp(i32::from(i16::MIN), i32::from(i16::MAX))
}

fn clamp16(sample: i32) -> i16 {
    i16::try_from(clamp(sample)).unwrap_or(0)
}

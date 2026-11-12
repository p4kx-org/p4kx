// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::path::Path;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{
    AnimationController, AnimationError, AnimationHeader, AnimationSource, CorruptAnimation,
    DbaAnimationSources, DbaHeaders, DecodedAnimationChannel,
};
use p4kx_media::{MediaRecord, Source, Status};
use serde_json::{Value, json};

use super::super::TestResult;
use super::super::shader_accounting::require;
use super::{Family, Lines};

#[derive(Default)]
pub(super) struct Totals {
    pub(super) complete: bool,
    pub(super) entries: u64,
    pub(super) chunks: u64,
    pub(super) clips: u64,
    pub(super) controllers: u64,
    pub(super) declared: u64,
    pub(super) decoded: u64,
    pub(super) partial: u64,
    pub(super) header_budget_refusals: u64,
}

pub(super) struct AnimationRows<'a> {
    pub(super) base: &'a Path,
    pub(super) records: &'a mut Lines,
    pub(super) source: &'a Source,
    pub(super) schema: &'a jsonschema::Validator,
    pub(super) totals: &'a mut Totals,
}

impl AnimationRows<'_> {
    pub(super) fn verify(&mut self, family: Family, name: &str, bytes: &[u8]) -> TestResult {
        let Ok(file) = ChunkFile::parse(bytes) else {
            return self.summary(family, "invalid-container", None, Status::Partial);
        };
        let count = file.chunks().len();
        for (ordinal, chunk) in file.chunks().iter().enumerate() {
            let row = self.next("animation-chunk", &[ordinal as u64], Status::Complete)?;
            check(
                &row.data,
                "/payload/type_id",
                &json!(chunk.chunk_type().id()),
            )?;
            check(&row.data, "/payload/version", &json!(chunk.version()))?;
            check(&row.data, "/payload/range/start", &json!(chunk.offset()))?;
            check(
                &row.data,
                "/payload/range/end_exclusive",
                &json!(chunk.offset() + chunk.size()),
            )?;
            self.totals.chunks += 1;
        }
        if matches!(family, Family::Aim) {
            return self.summary(family, "opaque-aim", Some(count), Status::Complete);
        }
        let before = (self.totals.clips, self.totals.declared, self.totals.partial);
        let complete = self.clips(family, name, &file)?;
        if complete && before.2 == self.totals.partial {
            self.headers(family, name, &file, before.0, before.1)?;
        }
        let status = if complete {
            Status::Complete
        } else {
            Status::Partial
        };
        self.summary(family, "clip-inventory", Some(count), status)
    }

    fn clips(&mut self, family: Family, name: &str, file: &ChunkFile<&[u8]>) -> TestResult<bool> {
        match family {
            Family::Caf => match AnimationSource::read_caf(name, file) {
                Ok(clip) => self.clip(&clip)?,
                Err(_) => return Ok(false),
            },
            Family::Dba => {
                let Ok(sources) = DbaAnimationSources::read(name, file) else {
                    return Ok(false);
                };
                for clip in sources {
                    let Ok(clip) = clip else { return Ok(false) };
                    self.clip(&clip)?;
                }
            }
            Family::Aim => return Err("AIM does not produce clip rows".into()),
        }
        Ok(true)
    }

    fn headers(
        &mut self,
        family: Family,
        name: &str,
        file: &ChunkFile<&[u8]>,
        clips_before: u64,
        keys_before: u64,
    ) -> TestResult {
        let inventory = match family {
            Family::Caf => {
                AnimationHeader::read_caf(name, file).map(|header| (1, header.keys as u64))
            }
            Family::Dba => DbaHeaders::read(name, file).map(|headers| {
                let clips = headers.animations.len() + headers.skipped.len();
                let mut keys = headers
                    .animations
                    .iter()
                    .map(|header| header.keys as u64)
                    .sum();
                for skipped in headers.skipped {
                    if let CorruptAnimation::TooManyKeys { keys: declared, .. } = skipped.reason {
                        keys += declared;
                        self.totals.header_budget_refusals += 1;
                    }
                }
                (clips as u64, keys)
            }),
            Family::Aim => return Err("AIM does not have decoded headers".into()),
        };
        match inventory {
            Ok((clips, keys)) => require(
                self.totals.clips - clips_before == clips
                    && self.totals.declared - keys_before == keys,
                "complete media clip and key totals agree with the independent header inventory",
            ),
            Err(AnimationError::Corrupt {
                source: CorruptAnimation::TooManyKeys { .. },
                ..
            }) => {
                self.totals.header_budget_refusals += 1;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    fn clip(&mut self, clip: &AnimationSource<'_>) -> TestResult {
        let ordinal = clip.ordinal() as u64;
        let metadata = clip.metadata();
        let timing = metadata.timing();
        let status = if timing.is_ok() {
            Status::Complete
        } else {
            Status::Partial
        };
        let row = self.next("animation-clip", &[ordinal], status)?;
        check(
            &row.data,
            "/payload/metadata_bytes",
            &json!(metadata.bytes().as_slice()),
        )?;
        check(&row.data, "/payload/flags", &json!(metadata.flags()))?;
        check(
            &row.data,
            "/payload/metadata_chunk_version",
            &json!(metadata.version()),
        )?;
        check(
            &row.data,
            "/payload/data_chunk_version",
            &json!(clip.data_chunk_version()),
        )?;
        check(
            &row.data,
            "/payload/metadata_range",
            &range(&metadata.range()),
        )?;
        check(
            &row.data,
            "/payload/controller_block_range",
            &range(&clip.controller_block_range()),
        )?;
        let database_name = clip.database_name().map(|name| {
            json!({
                "utf8": std::str::from_utf8(name.bytes()).ok(),
                "range": range(&name.range())
            })
        });
        check(&row.data, "/payload/database_name", &json!(database_name))?;
        let expected = timing.ok().map(|timing| {
            json!({
                "frames_per_second": timing.frames_per_second,
                "end_frame": timing.end_frame,
                "additive": timing.additive
            })
        });
        check(&row.data, "/payload/timing", &json!(expected))?;
        self.totals.clips += 1;
        for controller in clip.controllers()? {
            self.controller(ordinal, &controller)?;
        }
        Ok(())
    }

    fn controller(&mut self, clip: u64, controller: &AnimationController<'_>) -> TestResult {
        let rotation = controller.decode_rotation();
        let translation = controller.decode_translation();
        let status = if rotation.is_ok() && translation.is_ok() {
            Status::Complete
        } else {
            Status::Partial
        };
        let row = self.next(
            "animation-controller",
            &[clip, controller.ordinal() as u64],
            status,
        )?;
        check(
            &row.data,
            "/payload/controller_id",
            &json!(controller.controller_id()),
        )?;
        check(
            &row.data,
            "/payload/id_range",
            &range(&controller.id_range()),
        )?;
        check(
            &row.data,
            "/payload/table_range",
            &range(&controller.table_range()),
        )?;
        for (side, original) in [
            ("rotation", controller.rotation()),
            ("translation", controller.translation()),
        ] {
            let channel = row
                .data
                .pointer(&format!("/payload/{side}"))
                .ok_or("controller channel")?;
            check(channel, "/key_count", &json!(original.key_count))?;
            check(channel, "/packed_format", &json!(original.packed_format))?;
            check(channel, "/time_offset", &json!(original.time_offset))?;
            check(channel, "/value_offset", &json!(original.value_offset))?;
            self.totals.declared += u64::from(original.key_count);
        }
        self.totals.controllers += 1;
        self.channel(&row.data, "rotation", rotation)?;
        self.channel(&row.data, "translation", translation)
    }

    fn channel<const N: usize>(
        &mut self,
        row: &Value,
        side: &str,
        decoded: Result<Option<DecodedAnimationChannel<N>>, CorruptAnimation>,
    ) -> TestResult {
        let samples = row
            .pointer(&format!("/payload/{side}/samples"))
            .ok_or("samples")?;
        let decoded = match decoded {
            Ok(Some(decoded)) => decoded,
            Ok(None) => return check(samples, "/layout", &json!("empty")),
            Err(CorruptAnimation::UnknownFormat { .. }) => {
                return check(samples, "/layout", &json!("unsupported"));
            }
            Err(_) => return check(samples, "/layout", &json!("malformed")),
        };
        check(samples, "/layout", &json!("decoded"))?;
        let count = decoded.keys.frames.len();
        let frames = decoded
            .keys
            .frames
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let values = decoded
            .keys
            .values
            .into_iter()
            .flatten()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        super::super::animation_samples::compare(
            self.base,
            samples,
            "frames",
            &frames,
            count,
            decoded.frame_range,
        )?;
        super::super::animation_samples::compare(
            self.base,
            samples,
            "values",
            &values,
            count * N,
            decoded.value_range,
        )?;
        self.totals.decoded += count as u64;
        Ok(())
    }

    fn summary(
        &mut self,
        family: Family,
        layout: &str,
        chunks: Option<usize>,
        status: Status,
    ) -> TestResult {
        let row = self.next("animation-entry", &[], status)?;
        check(&row.data, "/payload/family", &json!(family.suffix()))?;
        check(&row.data, "/payload/layout", &json!(layout))?;
        check(&row.data, "/payload/chunk_count", &json!(chunks))
    }

    fn next(
        &mut self,
        namespace: &str,
        indices: &[u64],
        status: Status,
    ) -> TestResult<MediaRecord> {
        let row: MediaRecord = self.records.next()?.ok_or("missing animation row")?;
        require(
            row.source.identity == self.source.identity
                && row.source.root == self.source.root
                && row.source.raw == self.source.raw,
            "each row keeps its exact physical source",
        )?;
        require(
            row.subresource.namespace == namespace && row.subresource.indices == indices,
            "original row order and identity",
        )?;
        require(
            row.status == status,
            "explicit row coverage agrees with original parser",
        )?;
        require(
            self.schema.is_valid(&serde_json::to_value(&row)?),
            "animation row satisfies the built-in schema",
        )?;
        self.totals.partial += u64::from(status != Status::Complete);
        Ok(row)
    }
}

fn check(actual: &Value, field: &str, expected: &Value) -> TestResult {
    require(
        actual.pointer(field) == Some(expected),
        "animation field differs from original data",
    )
}

fn range(extent: &Range<u64>) -> Value {
    json!({
        "basis": {"type": "entry"},
        "start": extent.start,
        "end_exclusive": extent.end
    })
}

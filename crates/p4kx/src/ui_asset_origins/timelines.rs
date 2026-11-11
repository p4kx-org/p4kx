// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use p4kx_extraction::gfx::{
    AssetOrigins, FailureOrigin, MovieSpan, ObservedAssets, SpanRuns, Sprite, SpriteOrigins,
    TagCount, TimelineOrigins,
};

use super::{Conversion, Payload, Projection, Site, aligned};

impl Projection {
    pub(super) fn timelines(&mut self, observed: &ObservedAssets<'_>) -> Result<()> {
        let assets = &observed.assets;
        let origins = &observed.origins;
        let scan = origins.timeline.scan;
        let complete = !origins.truncated;
        self.frames("/frames", &origins.timeline)?;
        self.counts("/tags", &assets.tag_counts, &origins.tags, scan, complete)?;
        self.counts(
            "/unread_tags",
            &assets.unread_tags,
            &origins.unread,
            scan,
            complete,
        )?;

        self.logical(
            "/unknown_tags",
            Site::Survey(scan),
            "unknownTags",
            Conversion,
            complete,
        )?;
        for (index, (_, &tag)) in aligned(&assets.unknown_tags, &origins.unknown)?.enumerate() {
            let row = format!("/unknown_tags/{index}");
            let header = Site::Header(tag);
            self.logical(&row, header, "unknownTag", Payload, true)?;
            self.datum(
                &row,
                header,
                &[
                    ("code", Payload),
                    ("offset", Conversion),
                    ("length", Payload),
                ],
            )?;
        }

        self.logical(
            "/failures",
            Site::Survey(scan),
            "diagnostics",
            Conversion,
            complete,
        )?;
        for (index, (_, &failure)) in aligned(&assets.failures, &origins.failures)?.enumerate() {
            self.failure(&format!("/failures/{index}"), failure)?;
        }

        self.logical(
            "/sprites",
            Site::Survey(scan),
            "sprites",
            Conversion,
            complete,
        )?;
        for (index, (sprite, origin)) in aligned(&assets.sprites, &origins.sprites)?.enumerate() {
            self.sprite(&format!("/sprites/{index}"), sprite, origin, origins)?;
        }
        Ok(())
    }

    fn counts(
        &mut self,
        path: &str,
        counts: &[TagCount],
        by_code: &BTreeMap<u16, SpanRuns>,
        scan: MovieSpan,
        complete: bool,
    ) -> Result<()> {
        self.logical(path, Site::Survey(scan), "tagCounts", Conversion, complete)?;
        for (index, count) in counts.iter().enumerate() {
            let spans = by_code
                .get(&count.code)
                .context("missing GFx tag count receipt")?;
            let first = spans
                .runs()
                .first()
                .context("empty GFx tag count receipt")?
                .first;
            let row = format!("{path}/{index}");
            let site = Site::Runs(count.code, spans);
            self.logical(&row, site, "tagCount", Conversion, complete)?;
            self.logical(
                &format!("{row}/count"),
                site,
                "tagCount",
                Conversion,
                complete,
            )?;
            self.logical(
                &format!("{row}/code"),
                Site::Bytes(first),
                "code",
                Payload,
                true,
            )?;
            self.logical(
                &format!("{row}/name"),
                Site::Bytes(first),
                "code",
                Conversion,
                true,
            )?;
        }
        Ok(())
    }

    fn frames(&mut self, path: &str, timeline: &TimelineOrigins) -> Result<()> {
        let site = if timeline.frames.runs().is_empty() {
            Site::Survey(timeline.scan)
        } else {
            Site::Runs(1, &timeline.frames)
        };
        self.logical(path, site, "showFrameCount", Conversion, timeline.complete)
    }

    fn sprite(
        &mut self,
        path: &str,
        sprite: &Sprite,
        receipt: &SpriteOrigins,
        assets: &AssetOrigins,
    ) -> Result<()> {
        let definition = Site::Body(receipt.definition);
        self.logical(path, definition, "sprite", Payload, true)?;
        self.datum(
            path,
            definition,
            &[("id", Payload), ("frame_count", Payload)],
        )?;
        self.export_name(path, receipt.definition, sprite.id, assets)?;

        let origin = &receipt.timeline;
        self.frames(&format!("{path}/frames"), origin)?;
        self.labels(path, sprite, origin)?;
        self.logical(
            &format!("{path}/placed"),
            Site::Survey(origin.scan),
            "placedIds",
            Conversion,
            origin.complete,
        )?;
        for (index, id) in sprite.timeline.placed.iter().enumerate() {
            let declarations = origin
                .placements
                .get(id)
                .context("missing GFx placement receipt")?;
            let uses = declarations
                .iter()
                .map(|&tag| self.decoded(Site::Body(tag), "characterId", Payload))
                .collect::<Result<Vec<_>>>()?;
            self.bind_decoded(&format!("{path}/placed/{index}"), uses, true)?;
        }
        Ok(())
    }

    fn labels(&mut self, path: &str, sprite: &Sprite, origin: &TimelineOrigins) -> Result<()> {
        self.logical(
            &format!("{path}/labels"),
            Site::Survey(origin.scan),
            "labels",
            Conversion,
            origin.complete,
        )?;
        self.logical(
            &format!("{path}/frame_labels"),
            Site::Survey(origin.scan),
            "labels",
            Conversion,
            origin.complete,
        )?;
        for (index, (_, label)) in aligned(&sprite.timeline.labels, &origin.labels)?.enumerate() {
            let tag = Site::Body(label.tag);
            self.logical(
                &format!("{path}/labels/{index}"),
                tag,
                "name",
                Payload,
                true,
            )?;
            let row = format!("{path}/frame_labels/{index}");
            self.logical(&row, tag, "frameLabel", Payload, true)?;
            self.logical(&format!("{row}/name"), tag, "name", Payload, true)?;
            let frame = vec![
                self.decoded(
                    Site::Survey(label.frame_prefix),
                    "showFramePrefix",
                    Conversion,
                )?,
                self.decoded(Site::Header(label.tag), "labelBoundary", Conversion)?,
            ];
            self.bind_decoded(&format!("{row}/frame"), frame, true)?;
        }
        Ok(())
    }

    fn failure(&mut self, path: &str, receipt: FailureOrigin) -> Result<()> {
        match receipt {
            FailureOrigin::Tag(tag) => {
                self.logical(path, Site::Body(tag), "diagnostic", Conversion, true)?;
                self.logical(
                    &format!("{path}/code"),
                    Site::Header(tag),
                    "code",
                    Payload,
                    true,
                )?;
                self.logical(
                    &format!("{path}/offset"),
                    Site::Header(tag),
                    "bodyOffset",
                    Conversion,
                    true,
                )?;
                self.logical(
                    &format!("{path}/error"),
                    Site::Body(tag),
                    "diagnostic",
                    Conversion,
                    true,
                )
            }
            FailureOrigin::Stream {
                header, available, ..
            } => {
                let actual = [
                    self.decoded(Site::Bytes(header), "retainedTagHeader", Conversion)?,
                    self.decoded(Site::Survey(available), "availableByteExtent", Conversion)?,
                ];
                self.bind_decoded(path, actual.to_vec(), true)?;
                for field in ["code", "offset", "error"] {
                    self.bind_decoded(&format!("{path}/{field}"), actual.to_vec(), true)?;
                }
                Ok(())
            }
            _ => anyhow::bail!("unrecognized original GFx diagnostic receipt"),
        }
    }
}

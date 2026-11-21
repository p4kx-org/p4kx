// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_gfx::{ObservedAssets, TagSite};

use super::ui_bitmap_decode::BitmapRead;
use super::ui_bitmap_sources;
use super::ui_extract::{ObservedUiExtraction, UiAssetFailure, UiAssetKind, UiExtraction};
use crate::output_file::write_error;
use crate::ui_jobs::{ArtifactFields, CommittedUiAsset, JobResult, Outcome};
use crate::{
    ConversionError, DirectoryCommit, DirectoryReceipt, FieldOrigin, FieldOrigins, FieldPath,
    OriginOutputAddress, SourceUse, SourceUseRole, SourceUses, UiMovie, UiMovieEvidence,
    UiSourceSite as Site,
};

use SourceUseRole::{Conversion, Payload, Selector};

pub(super) fn complete<'owner>(
    movie: &UiMovie<'_>,
    publication: &'owner DirectoryReceipt<'_>,
    observed: ObservedAssets<'_>,
    jobs: Vec<JobResult<'owner, '_>>,
) -> Result<ObservedUiExtraction, ConversionError> {
    finish(movie, publication, observed, jobs)
        .map_err(|source| write_error(publication.path(), source))
}

fn finish<'owner>(
    movie: &UiMovie<'_>,
    publication: &'owner DirectoryReceipt<'_>,
    observed: ObservedAssets<'_>,
    jobs: Vec<JobResult<'owner, '_>>,
) -> io::Result<ObservedUiExtraction> {
    let uses = SourceUses::new();
    let source = UiMovieEvidence::observe(movie, &uses).map_err(io::Error::other)?;
    let mut report = Report {
        source,
        uses,
        fields: FieldOrigins::new(),
        decoded: &observed,
    };
    report.describe_input()?;
    let mut results = ExtractionResults::collect(&mut report, jobs)?;
    report.collections(results.written_uses, results.failed_uses)?;
    let inventory = report.uses.freeze().map_err(io::Error::other)?;
    let published = publication.publish(results.receipts, &inventory, results.artifact_fields)?;
    let receipt = if published {
        Some(
            publication
                .path()
                .file_name()
                .ok_or_else(|| invalid("directory receipt has no basename"))?
                .into(),
        )
    } else {
        None
    };
    let fields = report.fields;
    results.output.tag_failures = observed.assets.failures;
    Ok(ObservedUiExtraction {
        extraction: results.output,
        fields,
        inventory,
        receipt,
    })
}

struct SummarySources {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl Default for SummarySources {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }
}

#[derive(Default)]
struct ExtractionResults<'owner> {
    output: UiExtraction,
    receipts: Vec<DirectoryCommit<'owner>>,
    artifact_fields: Vec<(usize, OriginOutputAddress, FieldOrigin)>,
    written_uses: SummarySources,
    failed_uses: SummarySources,
}

impl<'owner> ExtractionResults<'owner> {
    fn collect(report: &mut Report<'_, '_>, jobs: Vec<JobResult<'owner, '_>>) -> io::Result<Self> {
        let mut result = Self::default();
        let tag_failures = &report.decoded.origins.failures;
        for (index, receipt) in tag_failures.iter().enumerate() {
            result
                .failed_uses
                .uses
                .extend(report.tag_failure(index, *receipt)?);
        }
        let mut bitmap_reads = Vec::new();
        let mut outcomes = Vec::new();
        for job in jobs {
            let group = bitmap_reads.len();
            bitmap_reads.push(job.bitmap);
            outcomes.extend(job.outcomes.into_iter().map(|outcome| (group, outcome)));
        }
        outcomes.sort_by_key(|(_, outcome)| outcome.order);
        for (group, outcome) in outcomes {
            let tag = report.definition(&outcome)?;
            match outcome.result {
                Ok(committed) => {
                    let bitmap = bitmap_reads.get(group).and_then(Option::as_ref);
                    result.committed(report, tag, outcome.order, bitmap, committed)?;
                }
                Err(error) => {
                    let index = tag_failures.len() + result.output.failures.len();
                    let (uses, complete) = report.failure(index, outcome.order, tag, &error)?;
                    result.failed_uses.uses.extend(uses);
                    result.failed_uses.complete &= complete;
                    result.output.failures.push(UiAssetFailure {
                        kind: outcome.kind,
                        id: outcome.id,
                        error,
                    });
                }
            }
        }
        Ok(result)
    }

    fn committed(
        &mut self,
        report: &mut Report<'_, '_>,
        tag: TagSite,
        order: usize,
        bitmap: Option<&BitmapRead<'_>>,
        committed: CommittedUiAsset<'owner>,
    ) -> io::Result<()> {
        let index = self.receipts.len();
        let (content, complete) = match committed.fields {
            ArtifactFields::Png => {
                let bitmap =
                    bitmap.ok_or_else(|| invalid("committed bitmap has no decoded input"))?;
                let (content, complete) = ui_bitmap_sources::contributions(
                    &report.source,
                    &report.decoded.origins,
                    &report.uses,
                    order,
                    bitmap,
                )?;
                self.artifact_fields.push((
                    index,
                    OriginOutputAddress::OpaquePayload,
                    report.uses.origin(&content, complete),
                ));
                (content, complete)
            }
            ArtifactFields::Json(fields) => {
                for field in fields {
                    let actual = if field.as_str() == "/version" {
                        report.tag_value(Site::Header(tag), "code", Conversion)?
                    } else {
                        report.tag_value(Site::Body(tag), field.as_str(), Payload)?
                    };
                    self.artifact_fields.push((
                        index,
                        OriginOutputAddress::JsonField { path: field },
                        report.uses.origin(&actual, report.source.is_bound()),
                    ));
                }
                let mut uses = report.tag_value(Site::Body(tag), "shapeData", Conversion)?;
                uses.push(report.decoded_use(Site::Header(tag), "code", Selector)?);
                (uses, report.source.is_bound())
            }
        };
        report.written(index, tag, &content, complete)?;
        self.written_uses.uses.extend(content);
        self.written_uses.complete &= complete;
        self.receipts.push(committed.receipt);
        self.output.written.push(committed.written);
        Ok(())
    }
}

pub(super) struct Report<'observed, 'movie> {
    pub(super) source: UiMovieEvidence,
    uses: SourceUses,
    fields: FieldOrigins,
    pub(super) decoded: &'observed ObservedAssets<'movie>,
}

impl Report<'_, '_> {
    fn describe_input(&mut self) -> io::Result<()> {
        let input = self.source.name_use().map_err(io::Error::other)?;
        self.bind("/movie", &[input], self.source.is_bound())?;
        self.bind("/target", &[], true)?;
        self.bind("/originReceipt", &[], true)
    }

    fn definition(&self, outcome: &Outcome<'_>) -> io::Result<TagSite> {
        let origins = &self.decoded.origins;
        let bitmaps = origins.bitmaps.len();
        let shapes = origins.shapes.len();
        let (sites, start) = match outcome.kind {
            UiAssetKind::Bitmap => (&origins.bitmaps, 0),
            UiAssetKind::Shape => (&origins.shapes, bitmaps),
            UiAssetKind::MorphShape => (&origins.morph_shapes, bitmaps + shapes),
        };
        outcome
            .order
            .checked_sub(start)
            .and_then(|index| sites.get(index))
            .copied()
            .ok_or_else(|| invalid("UI artifact has no physical GFx definition"))
    }

    fn written(
        &mut self,
        index: usize,
        tag: TagSite,
        content: &[SourceUse],
        complete: bool,
    ) -> io::Result<()> {
        let path = format!("/extracted/{index}");
        let kind = self.tag_value(Site::Header(tag), "code", Conversion)?;
        let id = self.tag_value(Site::Body(tag), "id", Payload)?;
        self.bind(&format!("{path}/kind"), &kind, self.source.is_bound())?;
        self.bind(&format!("{path}/id"), &id, self.source.is_bound())?;
        let name = kind.into_iter().chain(id).collect::<Vec<_>>();
        self.bind(&format!("{path}/file"), &name, self.source.is_bound())?;
        self.bind(&format!("{path}/size"), content, complete)?;
        let all = name
            .into_iter()
            .chain(content.iter().cloned())
            .collect::<Vec<_>>();
        self.bind(&path, &all, complete)
    }

    fn collections(
        &mut self,
        mut written: SummarySources,
        mut failed: SummarySources,
    ) -> io::Result<()> {
        let survey = Site::Survey(self.decoded.origins.timeline.scan);
        written
            .uses
            .extend(self.tag_value(survey, "definitionSurvey", Selector)?);
        failed
            .uses
            .extend(self.tag_value(survey, "failureSurvey", Selector)?);
        let complete = self.source.is_bound() && !self.decoded.origins.truncated;
        self.bind("/extracted", &written.uses, complete && written.complete)?;
        self.bind("/failed", &failed.uses, complete && failed.complete)
    }

    pub(super) fn bind(
        &mut self,
        path: &str,
        uses: &[SourceUse],
        complete: bool,
    ) -> io::Result<()> {
        self.fields
            .insert(
                FieldPath::parse(path).map_err(io::Error::other)?,
                self.uses.origin(uses, complete),
            )
            .map_err(io::Error::other)
    }

    pub(super) fn decoded_use(
        &self,
        site: Site<'_>,
        field: &str,
        role: SourceUseRole,
    ) -> io::Result<SourceUse> {
        self.source
            .decoded(site, field, role)
            .map_err(io::Error::other)
    }

    pub(super) fn tag_value(
        &self,
        site: Site<'_>,
        field: &str,
        role: SourceUseRole,
    ) -> io::Result<Vec<SourceUse>> {
        let mut uses = vec![self.decoded_use(site, field, role)?];
        self.source.append_decoding(&mut uses);
        Ok(uses)
    }
}

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

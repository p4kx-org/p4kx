// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use anyhow::Result;
use p4kx_export::{
    RecordExportOptions, RecordFilePlans, RecordForm, write_annotated_ndjson_with_origins,
    write_planned_record_export_with_origins, write_record_stream_with_origins,
};
use p4kx_extraction::{
    ObservedTagTree, RecordExportSources, RecordFilePlan, RecordLocalization,
    export_annotated_combined, export_annotated_combined_with_receipt,
    export_annotated_record_json_files, export_annotated_record_xml_files,
    export_planned_record_json_files_with_origins, export_planned_record_xml_files_with_origins,
};

use super::tags::RecordExportTags;
use super::{ExportedRecords, Job};

impl Job<'_, '_> {
    pub(super) fn publish(
        &self,
        options: &RecordExportOptions<'_>,
        target: &Path,
        combined: Option<&str>,
        localization: Option<&RecordLocalization<'_>>,
    ) -> Result<ExportedRecords> {
        let physical_options = RecordExportOptions {
            texts: None,
            tags: None,
            ..*options
        };
        let tags = self.tags.and_then(RecordExportTags::observed);
        let sources = RecordExportSources::new(self.input.entry(), localization, tags);
        if self.as_json {
            return self.publish_tracked(sources, &physical_options, target, combined);
        }
        if let Some(name) = combined {
            return Ok(ExportedRecords::Files(export_annotated_combined(
                self.input.entry(),
                self.forge,
                self.records,
                &target.join(name),
                &physical_options,
                localization,
                tags,
            )?));
        }
        let observed = match self.form {
            RecordForm::Json => Some(export_annotated_record_json_files(
                self.input.entry(),
                self.forge,
                self.records,
                target,
                &physical_options,
                localization,
                tags,
            )?),
            RecordForm::Xml => Some(export_annotated_record_xml_files(
                self.input.entry(),
                self.forge,
                self.records,
                target,
                &physical_options,
                localization,
                tags,
            )?),
            _ => None,
        };
        if let Some(report) = observed {
            return Ok(ExportedRecords::Files(report));
        }
        Ok(self.publish_stream(options, target, localization, tags))
    }

    fn publish_tracked(
        &self,
        sources: RecordExportSources<'_, '_>,
        options: &RecordExportOptions<'_>,
        target: &Path,
        combined: Option<&str>,
    ) -> Result<ExportedRecords> {
        let report = match (combined, self.form) {
            (Some(name), _) => export_annotated_combined_with_receipt(
                sources,
                self.forge,
                self.records,
                &target.join(name),
                options,
            )?,
            (None, RecordForm::Json) => export_planned_record_json_files_with_origins(
                sources,
                self.forge,
                &RecordFilePlan::new(self.forge),
                self.records,
                target,
                options,
            )?,
            (None, RecordForm::Xml) => export_planned_record_xml_files_with_origins(
                sources,
                self.forge,
                &RecordFilePlan::new(self.forge),
                self.records,
                target,
                options,
            )?,
            _ => {
                return Ok(ExportedRecords::Summary(
                    write_planned_record_export_with_origins(
                        sources,
                        self.forge,
                        &RecordFilePlans::default(),
                        self.records,
                        self.form,
                        target,
                        options,
                    ),
                ));
            }
        };
        Ok(ExportedRecords::Files(report))
    }

    fn publish_stream(
        &self,
        options: &RecordExportOptions<'_>,
        target: &Path,
        localization: Option<&RecordLocalization<'_>>,
        tags: Option<&ObservedTagTree>,
    ) -> ExportedRecords {
        let summary = if self.form == RecordForm::Ndjson {
            write_annotated_ndjson_with_origins(
                self.input.entry(),
                self.forge,
                self.records,
                target,
                options,
                localization,
                tags,
            )
        } else {
            write_record_stream_with_origins(
                self.input.entry(),
                self.forge,
                self.records,
                self.form,
                target,
                options,
                localization,
            )
        };
        ExportedRecords::Summary(summary)
    }
}

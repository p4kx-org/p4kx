// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write as _};
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, FieldOrigins, OriginArtifactBinding, OriginDigestWriter,
    SourceInventory,
};
use serde::Serialize;

use crate::chf_origin_fields::{ChfOrigins, ProjectedChfOrigins};
use crate::file_receipt::{FileReceipt, StdoutReceipt};
use crate::json;

pub(crate) struct ChfPublication<'artifact> {
    pub(crate) bytes: &'artifact [u8],
    pub(crate) fields: ProjectedChfOrigins,
    pub(crate) inventory: SourceInventory,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChfPublicationSummary {
    origin_receipt: String,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl ChfPublication<'_> {
    pub(crate) fn file(
        &self,
        input: &Path,
        output: &Path,
        archive: Option<&Path>,
        artifact_protection: Option<&ArchiveWriteProtection>,
        cancel: &CancelToken,
    ) -> Result<String> {
        let sources = [Some(input), archive];
        let sources: Vec<_> = sources.into_iter().flatten().collect();
        let receipt_protection = ArchiveWriteProtection::for_sources(&sources, cancel.flag())?;
        let receipt = FileReceipt::new_with_cancel(
            output,
            artifact_protection,
            Some(&receipt_protection),
            cancel.clone(),
        )?;
        receipt
            .publish(
                |writer| writer.write_all(self.bytes),
                |writer, binding, ()| self.fields.write(writer, binding, &self.inventory),
            )
            .with_context(|| format!("cannot write {}", output.display()))?;
        let name = receipt
            .path()
            .file_name()
            .context("missing origin companion basename")?;
        Ok(json::path(Path::new(name)))
    }

    pub(crate) fn stdout(
        &self,
        input: &Path,
        archive: Option<&Path>,
        origin_out: Option<&Path>,
        origin_stderr: bool,
        cancel: &CancelToken,
    ) -> Result<()> {
        let sources = [Some(input), archive];
        let sources: Vec<_> = sources.into_iter().flatten().collect();
        let protection = origin_out
            .map(|_| ArchiveWriteProtection::for_sources(&sources, cancel.flag()))
            .transpose()?;
        let durable = origin_out
            .map(|path| StdoutReceipt::new(path, protection.as_ref()))
            .transpose()?;
        let mut writer = OriginDigestWriter::new(BufWriter::new(io::stdout().lock()));
        writer.write_all(self.bytes)?;
        let binding = OriginArtifactBinding::Stdout {
            digest: writer.finish()?,
        };
        if let Some(durable) = durable {
            durable.publish(|writer| self.fields.write(writer, &binding, &self.inventory))?;
        }
        if origin_stderr {
            self.fields.write(
                &mut BufWriter::new(io::stderr().lock()),
                &binding,
                &self.inventory,
            )?;
        }
        Ok(())
    }

    pub(crate) fn summary(
        &self,
        origin_receipt: String,
        origins: &ChfOrigins,
    ) -> Result<ChfPublicationSummary> {
        let field_origins = origins.summary_fields(&self.fields)?;
        Ok(ChfPublicationSummary {
            origin_receipt,
            field_origins,
            provenance: origins.inventory()?,
        })
    }
}

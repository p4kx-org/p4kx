// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};

use p4kx_extraction::Archive;
use p4kx_media::{
    Capability, EntryInput, ExportOptions, Index, InputReceipt, MediaError, MediaKind, MediaWriter,
    OriginalIdentifier, Projection, Provider, ProviderRegistry, RequiredInput, Source,
    SourceContext, Status, Subresource,
};

use super::support::{self, TestResult};

pub(super) struct InputProvider {
    pub(super) uses: u32,
    pub(super) rereads: u32,
    pub(super) prefix: Option<u64>,
    pub(super) probe: bool,
    pub(super) visits: Arc<Mutex<Vec<usize>>>,
}

impl Provider for InputProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Texture,
            "synthetic.required-input",
            "1.0.0",
            "selected raw sources and explicit required-input evidence",
        )
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        self.visits
            .lock()
            .map_err(|_| MediaError::Contract("synthetic visit lock poisoned".into()))?
            .push(context.entry.index());
        if context.entry.path() != "owner.dds" {
            return Ok(Projection::Skipped);
        }
        let query = context.directory.lookup_input(
            OriginalIdentifier::new_with_path("owner.dds.1"),
            "owner.dds.1",
            Some(MediaKind::Texture),
            writer,
        )?;
        let mut receipts = Vec::<InputReceipt>::new();
        let mut observed = Vec::<Vec<u8>>::new();
        if let Some(token) = query.input {
            for use_index in 0..self.uses {
                let use_at = Subresource::new("synthetic.input", vec![u64::from(use_index)]);
                let receipt =
                    context.with_required_input(&token, &use_at, writer, |input, writer| {
                        if self.probe {
                            probe_input(context, &token, &use_at, input, writer)?;
                        }

                        for _ in 0..self.rereads {
                            input.seek(SeekFrom::Start(0)).map_err(input_error)?;
                            let mut bytes = Vec::new();
                            if let Some(limit) = self.prefix {
                                (&mut *input)
                                    .take(limit)
                                    .read_to_end(&mut bytes)
                                    .map_err(input_error)?;
                            } else {
                                input.read_to_end(&mut bytes).map_err(input_error)?;
                            }
                            observed.push(bytes);
                        }
                        Ok(input.receipt().clone())
                    })?;
                receipts.push(receipt);
            }
        }
        writer.record(
            &Subresource::new("synthetic.inputs", vec![0]),
            Status::Complete,
            &serde_json::json!({
                "reference": query.reference, "inputs": receipts, "bytes": observed,
            }),
            &[],
        )?;
        Ok(Projection::Written)
    }
}

fn probe_input(
    context: &SourceContext<'_>,
    token: &EntryInput<'_>,
    use_at: &Subresource,
    input: &mut RequiredInput<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let failure = input
        .seek(SeekFrom::End(1))
        .err()
        .ok_or_else(|| MediaError::Contract("invalid private seek succeeded".into()))?;
    let cause = failure
        .get_ref()
        .and_then(|cause| cause.downcast_ref::<MediaError>());
    if !matches!(cause, Some(MediaError::Io { path, source })
        if path == Path::new("owner.dds.1")
            && source.kind() == std::io::ErrorKind::InvalidInput)
    {
        return Err(MediaError::Contract(
            "private input exposed a staging path".into(),
        ));
    }
    let failure = context
        .with_required_input(token, use_at, writer, |_nested, _writer| Ok(()))
        .err()
        .ok_or_else(|| MediaError::Contract("private same-input reentry succeeded".into()))?;
    if !matches!(failure, MediaError::Contract(_)) {
        return Err(MediaError::Contract(
            "private reentry lost its typed refusal".into(),
        ));
    }
    Ok(())
}

fn input_error(source: std::io::Error) -> MediaError {
    MediaError::Io {
        path: "synthetic-required-input".into(),
        source,
    }
}

pub(super) fn run(
    archive: &Archive,
    output: &Path,
    options: &ExportOptions,
    uses: u32,
    rereads: u32,
) -> Result<(Index, Vec<usize>), Box<dyn Error>> {
    let visits = Arc::new(Mutex::new(Vec::new()));
    let providers = ProviderRegistry::try_new(vec![Box::new(InputProvider {
        uses,
        rereads,
        prefix: None,
        probe: false,
        visits: Arc::clone(&visits),
    })])?;
    let index = support::published(p4kx_media::export_with(
        archive, output, options, &providers,
    )?)?;
    let visits = visits
        .lock()
        .map_err(|_| "synthetic visit lock poisoned")?
        .clone();
    Ok((index, visits))
}

pub(super) fn sources(output: &Path, index: &Index) -> Result<Vec<Source>, Box<dyn Error>> {
    fs::read_to_string(
        output
            .join(&index.artifact_base)
            .join(&index.source_manifest.path),
    )?
    .lines()
    .map(|line| serde_json::from_str(line).map_err(Into::into))
    .collect()
}

pub(super) fn row(output: &Path, index: &Index) -> Result<serde_json::Value, Box<dyn Error>> {
    support::records(output, index, "texture.jsonl")?
        .into_iter()
        .next()
        .map(|row| row.data)
        .ok_or_else(|| "required-input row".into())
}

pub(super) fn has_limit(error: &(dyn Error + 'static), name: &str) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if matches!(error.downcast_ref::<MediaError>(), Some(MediaError::Limit { budget, .. }) if *budget == name)
        {
            return true;
        }
        current = error.source();
    }
    false
}

pub(super) fn no_private_spool(output: &Path, index: &Index) -> TestResult {
    for child in fs::read_dir(output.join(&index.artifact_base))? {
        assert!(
            !child?.file_name().to_string_lossy().starts_with(".entry-"),
            "private input staging never enters the published generation",
        );
    }
    Ok(())
}

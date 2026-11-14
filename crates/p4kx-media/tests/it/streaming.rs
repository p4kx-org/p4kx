// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{CancelToken, ConflictPolicy};
use p4kx_media::{
    ByteOrder, Capability, ChunkLayout, DataType, ExportOptions, MediaError, MediaKind,
    MediaWriter, Projection, Provider, ProviderRegistry, SourceContext, Status, Subresource,
};

use super::support::{self, TestResult};

const BLOCK: usize = 64 << 10;
const PAYLOAD: u64 = (4 * BLOCK + 17) as u64;

struct Generated {
    remaining: u64,
    reads: Arc<AtomicUsize>,
    maximum_request: Arc<AtomicUsize>,
    cancel: Option<CancelToken>,
}

impl Read for Generated {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.maximum_request
            .fetch_max(buffer.len(), Ordering::SeqCst);
        let read = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if read == 2
            && let Some(token) = &self.cancel
        {
            token.cancel();
        }
        let count = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        buffer
            .get_mut(..count)
            .ok_or_else(|| io::Error::other("generated read range"))?
            .fill(0x5a);
        self.remaining -= count as u64;
        Ok(count)
    }
}

struct StreamProvider {
    reads: Arc<AtomicUsize>,
    maximum_request: Arc<AtomicUsize>,
    cancel: Option<CancelToken>,
}

impl Provider for StreamProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Animation,
            "synthetic.large-stream",
            "1.0.0",
            "central source retention plus generated bounded test chunks",
        )
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let mut input = Generated {
            remaining: PAYLOAD,
            reads: self.reads.clone(),
            maximum_request: self.maximum_request.clone(),
            cancel: self.cancel.clone(),
        };
        let chunk = writer.chunk(
            ChunkLayout::new(
                DataType::U8,
                ByteOrder::Little,
                PAYLOAD,
                "synthetic unsigned samples; original order; exclusive bounds",
            ),
            &mut input,
        )?;
        writer.record(
            &Subresource::new("samples", vec![0]),
            Status::Complete,
            &chunk,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

#[test]
fn large_chunks_stream_through_bounded_actual_reader_requests() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let reads = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let providers = ProviderRegistry::try_new(vec![Box::new(StreamProvider {
        reads: reads.clone(),
        maximum_request: maximum.clone(),
        cancel: None,
    })])?;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &providers,
    )?)?;
    assert!(
        reads.load(Ordering::SeqCst) > 4,
        "reader crossed multiple actual buffer boundaries"
    );
    assert!(
        maximum.load(Ordering::SeqCst) <= BLOCK,
        "writer never requested an unbounded read"
    );
    let rows = support::records(&output, &index, "animation.jsonl")?;
    let path = rows
        .first()
        .and_then(|record| record.data.pointer("/artifact/path"))
        .and_then(serde_json::Value::as_str)
        .ok_or("typed chunk path")?;
    let mut file = fs::File::open(output.join(index.artifact_base).join(path))?;
    assert_eq!(file.metadata()?.len(), PAYLOAD);
    let mut buffer = vec![0; BLOCK];
    let mut size = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        assert!(
            buffer
                .get(..count)
                .ok_or("written range")?
                .iter()
                .all(|&byte| byte == 0x5a)
        );
        size += count as u64;
    }
    assert_eq!(size, PAYLOAD);
    Ok(())
}

#[test]
fn cancellation_inside_a_real_chunk_read_preserves_the_previous_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let previous = fs::read(output.join("media-index.json"))?;
    let reads = Arc::new(AtomicUsize::new(0));
    let token = CancelToken::new();
    let providers = ProviderRegistry::try_new(vec![Box::new(StreamProvider {
        reads: reads.clone(),
        maximum_request: Arc::new(AtomicUsize::new(0)),
        cancel: Some(token.clone()),
    })])?;
    let options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        cancel: token,
        ..ExportOptions::default()
    };
    let error = p4kx_media::export_with(&archive, &output, &options, &providers)
        .err()
        .ok_or("interrupted chunk published")?;
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    let MediaError::Source { cause, .. } = error else {
        return Err("chunk source context".into());
    };
    assert!(matches!(*cause, MediaError::Cancelled));
    assert_eq!(fs::read(output.join("media-index.json"))?, previous);
    Ok(())
}

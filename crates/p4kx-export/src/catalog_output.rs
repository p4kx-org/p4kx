// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_extraction::CancelToken;
use zstd::Encoder;

use crate::CatalogError;
use crate::contract::Catalog;
use crate::options::{CatalogRun, CatalogStage};

/// The zstd level a compressed catalog is written at.
pub const ZSTD_LEVEL: i32 = 19;

/// The base-2 logarithm of the largest window a compressed catalog may use; the frame declares
/// its content size, so the window is no larger than the catalog.
pub const ZSTD_WINDOW_LOG: u32 = 27;

const BUFFER: usize = 64 * 1024;

/// How the catalog JSON is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    /// Plain UTF-8 JSON.
    #[default]
    None,
    /// One zstd frame with a content checksum, the content size and a window of at most
    /// 2^[`ZSTD_WINDOW_LOG`].
    Zstd {
        /// The compression level, 1 to 22.
        level: i32,
    },
}

impl Compression {
    /// zstd at [`ZSTD_LEVEL`].
    pub const ZSTD: Self = Self::Zstd { level: ZSTD_LEVEL };

    /// Returns [`ZSTD`](Self::ZSTD) for a path ending in `.zst`, ignoring case, otherwise
    /// [`None`](Self::None).
    #[must_use]
    pub fn for_path(path: &Path) -> Self {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zst"))
        {
            Self::ZSTD
        } else {
            Self::None
        }
    }
}

/// A catalog streamed into its writer.
pub(crate) struct Written<W> {
    pub(crate) sink: W,
    pub(crate) size: u64,
    pub(crate) compressed: Option<u64>,
    pub(crate) staged_row_bytes: u128,
}

/// Serialises `catalog` as JSON followed by a newline into `out`, through the compression.
pub(crate) fn emit<W: Write>(
    catalog: &Catalog<'_>,
    out: W,
    compression: Compression,
    run: CatalogRun<'_, '_>,
) -> Result<Written<W>, CatalogError> {
    let cancel = run.options.cancel;
    let staged_row_bytes = catalog.placements.staged_row_bytes()
        + u128::from(catalog.configurations.durability.stored_bytes())
        + u128::from(catalog.definitions.blueprints.stored_bytes())
        + u128::from(catalog.definitions.particle_libraries.stored_bytes())
        + u128::from(catalog.definitions.contract_generators.stored_bytes())
        + u128::from(catalog.definitions.contract_templates.stored_bytes())
        + u128::from(catalog.definitions.mission_broker_entries.stored_bytes())
        + u128::from(
            catalog
                .configurations
                .entity_configurations
                .alignment
                .item_ports
                .stored_bytes(),
        )
        + u128::from(
            catalog
                .configurations
                .entity_configurations
                .interactions
                .stored_bytes(),
        )
        + u128::from(
            catalog
                .projections
                .weapon_player_animation_candidates
                .stored_bytes(),
        );
    match compression {
        Compression::None => {
            run.begin(CatalogStage::WriteJson);
            let mut counted = Counting::new(out, cancel);
            write_json(catalog, &mut counted, cancel)?;
            Ok(Written {
                size: counted.count,
                sink: counted.inner,
                compressed: None,
                staged_row_bytes,
            })
        }
        Compression::Zstd { level } => {
            run.begin(CatalogStage::MeasureJson);
            let length = measure(catalog, cancel)?;
            run.begin(CatalogStage::WriteJson);
            let encoder =
                encoder(Counting::new(out, None), level, length).map_err(CatalogError::Write)?;
            let mut counted = Counting::new(encoder, cancel);
            write_json(catalog, &mut counted, cancel)?;
            let size = counted.count;
            let packed = counted.inner.finish().map_err(CatalogError::Write)?;
            Ok(Written {
                size,
                compressed: Some(packed.count),
                sink: packed.inner,
                staged_row_bytes,
            })
        }
    }
}

fn encoder<'a, W: Write>(sink: W, level: i32, length: u64) -> io::Result<Encoder<'a, W>> {
    let mut encoder = Encoder::new(sink, level)?;
    encoder.set_pledged_src_size(Some(length))?;
    encoder.include_contentsize(true)?;
    encoder.include_checksum(true)?;
    encoder.long_distance_matching(true)?;
    encoder.window_log(ZSTD_WINDOW_LOG)?;
    Ok(encoder)
}

fn write_json(
    catalog: &Catalog<'_>,
    out: &mut impl Write,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let mut buffered = BufWriter::with_capacity(BUFFER, out);
    let written = serde_json::to_writer(&mut buffered, catalog)
        .map_err(|error| {
            let failure = catalog
                .placements
                .take_row_spool_failure()
                .or_else(|| catalog.configurations.durability.take_failure())
                .or_else(|| catalog.definitions.blueprints.take_failure())
                .or_else(|| catalog.definitions.contract_generators.take_failure())
                .or_else(|| catalog.definitions.contract_templates.take_failure())
                .or_else(|| catalog.definitions.mission_broker_entries.take_failure())
                .or_else(|| catalog.definitions.particle_libraries.take_failure())
                .or_else(|| {
                    catalog
                        .configurations
                        .entity_configurations
                        .alignment
                        .item_ports
                        .take_failure()
                })
                .or_else(|| {
                    catalog
                        .configurations
                        .entity_configurations
                        .interactions
                        .take_failure()
                })
                .or_else(|| {
                    catalog
                        .projections
                        .weapon_player_animation_candidates
                        .take_failure()
                });
            if let Some(failure) = failure {
                return CatalogError::from(failure);
            }
            if error.is_io() {
                CatalogError::Write(error.into())
            } else {
                CatalogError::Json(error)
            }
        })
        .and_then(|()| buffered.write_all(b"\n").map_err(CatalogError::Write))
        .and_then(|()| buffered.flush().map_err(CatalogError::Write));
    match written {
        Err(CatalogError::Write(_)) if cancel.is_some_and(CancelToken::is_cancelled) => {
            Err(CatalogError::Cancelled)
        }
        other => other,
    }
}

/// Counts the bytes that pass and stops with an error once cancelled.
struct Counting<'c, W> {
    inner: W,
    count: u64,
    cancel: Option<&'c CancelToken>,
}

impl<'c, W> Counting<'c, W> {
    const fn new(inner: W, cancel: Option<&'c CancelToken>) -> Self {
        Self {
            inner,
            count: 0,
            cancel,
        }
    }
}

impl<W: Write> Write for Counting<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(io::Error::other("the catalog write was cancelled"));
        }
        let written = self.inner.write(bytes)?;
        self.count = self
            .count
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Serialises `catalog` into a sink and returns the length of its JSON, newline included.
fn measure(catalog: &Catalog<'_>, cancel: Option<&CancelToken>) -> Result<u64, CatalogError> {
    let mut counted = Counting::new(io::sink(), cancel);
    write_json(catalog, &mut counted, cancel)?;
    Ok(counted.count)
}

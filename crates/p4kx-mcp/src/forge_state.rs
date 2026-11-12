// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex, PoisonError};

use p4kx_extraction::{DataForge, DataForgeLoadError, DataForgeOrigin, LoadedDataForge};

use crate::error::{ToolError, chain};

/// The selection of the last Query run: the Query text and the indexes of the Records it chose.
type Selection = Option<(String, Arc<[u32]>)>;

/// The DataForge of one input, read on first use, and the selection of the last Query run over
/// it.
#[derive(Debug, Default)]
pub(crate) struct ForgeState {
    dataforge: Mutex<Option<Result<Arc<LoadedDataForge>, String>>>,
    selection: Mutex<Selection>,
}

impl ForgeState {
    /// Runs `work` against the DataForge with the input path as text, reading the DataForge with
    /// `read` on first use.
    pub(crate) fn with_origin<T, F>(
        &self,
        read: impl FnOnce() -> Result<LoadedDataForge, DataForgeLoadError>,
        path: String,
        work: F,
    ) -> Result<T, ToolError>
    where
        T: Send + 'static,
        F: FnOnce(&DataForge<'_>, String, &DataForgeOrigin) -> Result<T, ToolError>
            + Send
            + 'static,
    {
        self.dataforge(read)?
            .with_origin(move |forge, origin| work(forge, path, origin))
            .map_err(|_| ToolError::Stopped)?
    }

    /// Reads the DataForge with `read` if that has not happened yet, without running any work on
    /// it.
    pub(crate) fn load(
        &self,
        read: impl FnOnce() -> Result<LoadedDataForge, DataForgeLoadError>,
    ) -> Result<(), ToolError> {
        self.dataforge(read).map(drop)
    }

    /// Returns the selection of the last Query run if it was of `query`.
    pub(crate) fn selection(&self, query: &str) -> Option<Arc<[u32]>> {
        let selection = self
            .selection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        selection
            .as_ref()
            .filter(|(text, _)| text == query)
            .map(|(_, records)| Arc::clone(records))
    }

    /// Keeps `records` as the selection of the last Query run, which was of `query`.
    pub(crate) fn keep_selection(&self, query: &str, records: Arc<[u32]>) {
        *self
            .selection
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((query.to_owned(), records));
    }

    /// Returns the DataForge, reading it on first use; calls meanwhile wait for that read. A
    /// DataForge that cannot be read is not read again, but one that failed for a reason of the
    /// disk, a changed Archive or a missing thread is read again by the next call.
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the lock is held through the read so that other calls wait for it"
    )]
    fn dataforge(
        &self,
        read: impl FnOnce() -> Result<LoadedDataForge, DataForgeLoadError>,
    ) -> Result<Arc<LoadedDataForge>, ToolError> {
        let mut slot = self
            .dataforge
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(loaded) = slot.as_ref() {
            return loaded.clone().map_err(ToolError::NoDataForge);
        }
        match read() {
            Ok(loaded) => {
                let loaded = Arc::new(loaded);
                *slot = Some(Ok(Arc::clone(&loaded)));
                Ok(loaded)
            }
            Err(error) => {
                let mut message = chain(&error);
                if let Some(hint) = crate::keys::hint(&error) {
                    message.push_str(". ");
                    message.push_str(hint);
                }
                if !error.is_transient() {
                    *slot = Some(Err(message.clone()));
                }
                Err(ToolError::NoDataForge(message))
            }
        }
    }
}

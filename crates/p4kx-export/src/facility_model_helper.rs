// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, AssemblyOptions, Geometry, ModelFailure, ProvidedModel, RoomModelHelper,
};
use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{
    CancelToken, DataForge, EntryOrigin, InputScope, PathIndex, SourceInput, SourceUse,
    SourceUseRole,
};

use super::super::authored_fields::RoomTransformUnavailable;
use super::node_inputs;
use super::routes::{self, Selection};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{EntryCapture, EntryReceipt};
use crate::{CatalogError, ExportAccountingError};

pub(super) struct RoomHelperModels<'archive, 'forge, 'capture> {
    index: &'archive PathIndex<'archive>,
    forge: &'forge DataForge<'forge>,
    capture: &'capture EntryCapture<'capture>,
    origins: &'capture CatalogOrigins,
    cancel: Option<&'capture CancelToken>,
    provider: Option<ArchiveModels<'archive>>,
    host: Option<RoomHelperHost>,
}

struct RoomHelperHost {
    record: u32,
    geometry: Result<Geometry, RoomTransformUnavailable>,
    model: Option<Result<ProvidedModel, RoomTransformUnavailable>>,
}

pub(super) struct ObservedRoomHelper {
    pub(super) value: Result<RoomModelHelper, RoomTransformUnavailable>,
    pub(super) inputs: Vec<SourceUse>,
}

impl std::fmt::Debug for RoomHelperModels<'_, '_, '_> {
    fn fmt(&self, format: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format
            .debug_struct("RoomHelperModels")
            .finish_non_exhaustive()
    }
}

impl<'archive, 'forge, 'capture> RoomHelperModels<'archive, 'forge, 'capture> {
    pub(super) const fn new(
        index: &'archive PathIndex<'archive>,
        forge: &'forge DataForge<'forge>,
        capture: &'capture EntryCapture<'capture>,
        origins: &'capture CatalogOrigins,
        cancel: Option<&'capture CancelToken>,
    ) -> Self {
        Self {
            index,
            forge,
            capture,
            origins,
            cancel,
            provider: None,
            host: None,
        }
    }

    pub(super) fn resolve(
        &mut self,
        record: u32,
        name: &str,
    ) -> Result<ObservedRoomHelper, CatalogError> {
        let mut inputs = Vec::new();
        let value = self.read(record, name, &mut inputs)?;
        Ok(ObservedRoomHelper { value, inputs })
    }

    fn read(
        &mut self,
        record: u32,
        name: &str,
        inputs: &mut Vec<SourceUse>,
    ) -> Result<Result<RoomModelHelper, RoomTransformUnavailable>, CatalogError> {
        if self.host.as_ref().is_none_or(|host| host.record != record) {
            self.host = Some(RoomHelperHost {
                record,
                geometry: self.geometry(record)?,
                model: None,
            });
        }
        let host = self
            .host
            .as_mut()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let geometry = match &host.geometry {
            Ok(geometry) => geometry,
            Err(state) => return Ok(Err(*state)),
        };
        let selector = geometry
            .model_field()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        inputs.push(
            self.origins
                .dataforge
                .at(selector, SourceUseRole::Selector)?,
        );
        let path = geometry.path().trim_start_matches(['/', '\\']);
        let under_data = format!("Data\\{path}");
        let archive = self.index.archive();
        let entry = match routes::select(self.index, [under_data.as_str(), path], self.cancel)? {
            Selection::One(entry) => entry,
            Selection::Missing => return Ok(Err(RoomTransformUnavailable::Unsupported)),
            Selection::Ambiguous(entries) => {
                for entry in entries {
                    let receipt = EntryReceipt::outer(self.capture, archive, entry)?;
                    inputs.extend_from_slice(receipt.physical_path()?.contributions().0);
                }
                return Ok(Err(RoomTransformUnavailable::Invalid));
            }
        };
        let receipt = EntryReceipt::outer(self.capture, archive, entry)?;
        let path = receipt.physical_path()?;
        inputs.extend_from_slice(path.contributions().0);
        if path_extension_is_cdf(entry.path()) {
            return Ok(Err(RoomTransformUnavailable::Unsupported));
        }
        let original = archive
            .entry_origin(entry)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let provider = self
            .provider
            .get_or_insert_with(|| ArchiveModels::new(archive).with_model_node_sources());
        let model = host
            .model
            .get_or_insert_with(|| read_model(provider, geometry.path()));
        routes::check_cancel(self.cancel)?;
        match model {
            Ok(model) => observe_helper(self.capture, &original, model, name, self.cancel, inputs),
            Err(state) => {
                receipt.attempted_payload()?;
                Ok(Err(*state))
            }
        }
    }

    fn geometry(
        &self,
        record: u32,
    ) -> Result<Result<Geometry, RoomTransformUnavailable>, CatalogError> {
        routes::check_cancel(self.cancel)?;
        let record = self
            .forge
            .record(record)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let Ok(tree) = record.tree() else {
            return Ok(Err(RoomTransformUnavailable::Unreadable));
        };
        Ok(p4kx_assembly::read_geometry(&tree).ok_or(RoomTransformUnavailable::Unsupported))
    }
}

fn path_extension_is_cdf(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("cdf"))
}

fn read_model(
    provider: &ArchiveModels<'_>,
    path: &str,
) -> Result<ProvidedModel, RoomTransformUnavailable> {
    use p4kx_assembly::ModelProvider as _;

    provider
        .model(path, AssemblyOptions::default().lod)
        .map_err(|error| match error {
            ModelFailure::Missing => RoomTransformUnavailable::Unsupported,
            _ => RoomTransformUnavailable::Unreadable,
        })
}

fn observe_helper(
    capture: &EntryCapture<'_>,
    original: &EntryOrigin,
    model: &ProvidedModel,
    name: &str,
    cancel: Option<&CancelToken>,
    inputs: &mut Vec<SourceUse>,
) -> Result<Result<RoomModelHelper, RoomTransformUnavailable>, CatalogError> {
    let Some(primary) = model
        .archive_sources()
        .iter()
        .find(|source| source.file() == ModelFile::Model)
    else {
        return Ok(Err(RoomTransformUnavailable::Unsupported));
    };
    let Some((origin, nodes)) = primary.origin().zip(primary.node_sources()) else {
        return Ok(Err(RoomTransformUnavailable::Unsupported));
    };
    capture.check_origin(origin)?;
    if !origin.binding().is_same_entry(&original.binding()) {
        return Ok(Err(RoomTransformUnavailable::Unsupported));
    }
    let scope = capture
        .source_uses()
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    node_inputs::select(&scope, &model.model, nodes, name, cancel, inputs)
}

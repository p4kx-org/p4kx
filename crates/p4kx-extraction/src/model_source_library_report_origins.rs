// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::EntryOrigin;
use p4kx_model::{LodSelection, MaterialReading, Model, ModelFile, PositionBox};

use super::{Coordinates, MaterialEvidence, ModelSource, ReceiptError};
use crate::model_info_materials::receipt::{
    MaterialReadInput, Observations, model_material_selector,
};
use crate::source_provenance::SourceCoordinateOmission;
use crate::{
    ConversionError, FieldOrigin, FieldOrigins, FieldPath, InputScope, ModelArchiveSource,
    PathIndex, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses, TextureVerdicts,
};

impl ModelSource {
    /// Reads the chosen levels of detail and observes the drawn material summary.
    ///
    /// # Errors
    /// Returns the original model-read error.
    pub fn from_index_with_drawn_material_summary_origins(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
    ) -> Result<Self, ConversionError> {
        Self::read_archive(
            index,
            path,
            lods,
            PositionBox::default(),
            MaterialReading::Drawn,
            &TextureVerdicts::new(),
            Coordinates::MaterialSummary,
        )
    }

    /// Addresses the selected model and material library fields in an output report.
    ///
    /// # Errors
    /// Returns an error when source evidence cannot be recorded or a bound library was not observed.
    pub fn model_and_library_report_origins(
        &self,
        uses: &SourceUses,
        model_path: &FieldPath,
        library_path: &FieldPath,
    ) -> Result<FieldOrigins, ConversionError> {
        let primary = self
            .archive_sources
            .iter()
            .find(|read| read.file() == ModelFile::Model)
            .and_then(ModelArchiveSource::origin);
        let observation = match &self.material_evidence {
            Some(MaterialEvidence::Summary(read)) => Some(read.as_ref()),
            _ => None,
        };
        let library_entry = observation.and_then(|read| read.library.as_ref());
        let mut fields = loaded_fields(&self.model, primary, observation, uses, library_path)?;
        write_report_fields(
            uses,
            &ReportInputs {
                model: &self.model,
                primary,
                library_entry,
            },
            model_path,
            library_path,
            &mut fields,
        )
        .map_err(material_error)?;
        Ok(fields)
    }
}

fn loaded_fields(
    model: &Model,
    primary: Option<&EntryOrigin>,
    observed: Option<&MaterialReadInput>,
    uses: &SourceUses,
    library_path: &FieldPath,
) -> Result<FieldOrigins, ConversionError> {
    if model.materials.library().is_none() {
        return Ok(FieldOrigins::new());
    }
    if let Some(observed) = observed {
        return observed.fields.project_loaded_summary(
            model,
            primary,
            observed.library.as_ref(),
            uses,
            library_path,
            None,
        );
    }
    if primary.is_some() {
        return Err(material_error(ReceiptError::MissingRows));
    }
    Observations::default().project_loaded_summary(model, None, None, uses, library_path, None)
}

struct ReportInputs<'source> {
    model: &'source Model,
    primary: Option<&'source EntryOrigin>,
    library_entry: Option<&'source EntryOrigin>,
}

fn write_report_fields(
    uses: &SourceUses,
    inputs: &ReportInputs<'_>,
    model_path: &FieldPath,
    library_path: &FieldPath,
    fields: &mut FieldOrigins,
) -> Result<(), ReceiptError> {
    let model_origin = directory_path(uses, inputs.primary)?;
    fields.insert(model_path.clone(), model_origin)?;

    let primary = inputs
        .primary
        .map(|entry| uses.observe(SourceInput::Archive(entry), InputScope::Payload))
        .transpose()?;
    let selector = primary
        .as_ref()
        .zip(model_material_selector(inputs.model))
        .map(|(scope, location)| scope.at(location, SourceUseRole::Selector))
        .transpose()?;
    let decoded_name = primary
        .as_ref()
        .zip(selector.as_ref())
        .map(|(scope, observed)| scope.at_observed_location(observed, SourceUseRole::Conversion))
        .transpose()?;
    let name = uses.origin(decoded_name.as_slice(), false);
    let selected = uses.origin(selector.as_slice(), false);
    let loaded = inputs.model.materials.library().is_some();
    let payload = library_payload(uses, inputs.library_entry, loaded)?;
    let status =
        uses.origin_with_origins([&selected, &uses.origin(payload.as_slice(), false)], false)?;
    let unknown = uses.origin(&[], false);
    for (field, origin) in [
        ("path", name),
        ("target", selected),
        ("status", status.clone()),
        ("problem", status.clone()),
        ("authored", if loaded { status } else { unknown.clone() }),
    ] {
        put(fields, library_path, field, origin)?;
    }
    if !loaded {
        put(fields, library_path, "single", unknown.clone())?;
        put(fields, library_path, "default_palette", unknown)?;
    }
    Ok(())
}

fn directory_path(
    uses: &SourceUses,
    entry: Option<&EntryOrigin>,
) -> Result<FieldOrigin, ReceiptError> {
    let Some(entry) = entry else {
        return Ok(uses.origin(&[], false));
    };
    let path = uses
        .observe(
            SourceInput::Archive(entry),
            InputScope::ArchiveDirectoryMetadata,
        )?
        .at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
    Ok(uses.origin(&[path], entry.ancestors().all(|source| source.is_some())))
}

fn library_payload(
    uses: &SourceUses,
    entry: Option<&EntryOrigin>,
    loaded: bool,
) -> Result<Option<SourceUse>, ReceiptError> {
    let Some(entry) = entry else {
        return Ok(None);
    };
    let role = if loaded {
        SourceUseRole::Conversion
    } else {
        SourceUseRole::Attempted
    };
    let observation = uses
        .observe(SourceInput::Archive(entry), InputScope::Payload)?
        .at_opaque(role, SourceCoordinateOmission::Unavailable)?;
    Ok(loaded.then_some(observation))
}

fn put(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> Result<(), ReceiptError> {
    let mut path = parent.clone();
    path.push_field(name)?;
    fields.insert(path, origin)?;
    Ok(())
}

fn material_error(error: ReceiptError) -> ConversionError {
    ConversionError::MaterialSourceEvidence(Box::new(error))
}

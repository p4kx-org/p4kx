// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::EntryOrigin;
use p4kx_model::{MaterialReadField, Model};

use super::{Observations, ReceiptError, model_material_selector};
use crate::material_source_info::MaterialSourceEvidence;
use crate::{ConversionError, FieldOrigins, FieldPath, SourceUses};

impl Observations {
    pub(crate) fn project_loaded_summary(
        &self,
        model: &Model,
        model_entry: Option<&EntryOrigin>,
        library_entry: Option<&EntryOrigin>,
        uses: &SourceUses,
        library_path: &FieldPath,
        count_path: Option<&FieldPath>,
    ) -> Result<FieldOrigins, ConversionError> {
        use MaterialReadField::{LibraryDefaultPalette, LibrarySingle, LibraryTotal};
        let project = || {
            let library = model.materials.library().ok_or(ReceiptError::MissingRows)?;
            let mut output = self.writer(
                uses,
                library_entry,
                model_entry.zip(model_material_selector(model)),
            )?;
            // The library lookup decisions are outside the loaded payload observation.
            output.complete = false;
            for (name, original) in [
                ("single", LibrarySingle),
                ("default_palette", LibraryDefaultPalette),
            ] {
                let mut path = library_path.clone();
                path.push_field(name)?;
                output.put(path.as_str(), 0, &[original])?;
            }
            if let Some(count_path) = count_path {
                output.put(count_path.as_str(), 0, &[LibraryTotal])?;
            }
            let shared = uses.shared_origin(output.selector.as_slice(), output.complete);
            let mut path = library_path.clone();
            path.push_field("authored")?;
            MaterialSourceEvidence::new(uses, output.scope.as_ref(), &shared).project(
                &library.authored,
                &path,
                &mut output.fields,
            )?;
            Ok(output.fields)
        };
        project()
            .map_err(|error: ReceiptError| ConversionError::MaterialSourceEvidence(Box::new(error)))
    }
}

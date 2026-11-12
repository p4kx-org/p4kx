// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use anyhow::{Result, ensure};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, FieldOrigins, FieldPath, InputScope, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use p4kx_screens::{
    CanvasInput, MovieInput, ObservedEntityScreens, Screen, ScreenInput, ValueInput,
};
use serde::Serialize;

use SourceUseRole::{Conversion, Payload, Selector};

#[path = "ui_screens_widget_origins.rs"]
mod widgets;

use self::widgets::Route;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Evidence {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl Evidence {
    pub(super) fn of(entry: &DataForgeEntry, observed: &ObservedEntityScreens) -> Result<Self> {
        let uses = SourceUses::new();
        let input = match entry.origin() {
            DataForgeOrigin::Archive(entry) => SourceInput::Archive(entry),
            _ => SourceInput::Unknown,
        };
        let scope = uses.observe(input, InputScope::Payload)?;
        let mut projection = Projection {
            scope,
            uses,
            fields: FieldOrigins::new(),
        };
        let inputs = observed.inputs();
        ensure!(
            observed.listing().screens.len() == inputs.screens().len()
                && observed.listing().movies.len() == inputs.movies().len(),
            "screen listing origin row counts",
        );
        projection.base(inputs.entity_record(), inputs.brand_style_record())?;
        for (index, (row, input)) in observed
            .listing()
            .screens
            .iter()
            .zip(inputs.screens())
            .enumerate()
        {
            projection.screen(index, row, input)?;
        }
        for (index, input) in inputs.movies().iter().enumerate() {
            projection.movie(index, input)?;
        }
        Ok(Self {
            field_origins: projection.fields,
            provenance: projection.uses.freeze()?,
        })
    }
}

struct Projection {
    scope: SourceScope,
    uses: SourceUses,
    fields: FieldOrigins,
}

impl Projection {
    fn base(&mut self, record: Option<u32>, brand_style: Option<u32>) -> Result<()> {
        self.put("/archive", &[], true)?;
        let path = self
            .scope
            .for_scope(InputScope::ArchiveDirectoryMetadata)
            .at(
                SourceLocation::Metadata {
                    field: "path".to_owned(),
                },
                Payload,
            )?;
        self.put("/entry", &[path], true)?;
        self.entity("/entity", record, true)?;
        if let Some(style) = brand_style {
            let choice = self.header(style, "fileName", Selector)?;
            let mut inputs = vec![choice];
            if let Some(index) = record {
                inputs.push(self.header(index, "recordName", Conversion)?);
            }
            self.put("/brand", &inputs, record.is_some())?;
        }
        self.put("/screens", &[], false)?;
        self.put("/movies", &[], false)?;
        Ok(())
    }

    fn entity(&mut self, path: &str, record: Option<u32>, complete: bool) -> Result<()> {
        if let Some(index) = record {
            let name = self.header(index, "recordName", Conversion)?;
            self.put(path, &[name], complete)
        } else {
            self.put(path, &[], false)
        }
    }

    fn screen(&mut self, index: usize, row: &Screen, input: &ScreenInput) -> Result<()> {
        let path = format!("/screens/{index}");
        let complete = input.is_root && input.entity_record == Some(input.reference.record_index);
        self.put(&path, &[], false)?;
        self.put(&format!("{path}/port"), &[], input.is_root)?;
        self.entity(
            &format!("{path}/entity"),
            input.entity_record,
            input.is_root,
        )?;
        let file = input
            .entity_record
            .map(|index| self.header(index, "fileName", Conversion))
            .transpose()?;
        self.put(
            &format!("{path}/entity_record_path"),
            file.as_slice(),
            input.is_root && file.is_some(),
        )?;

        let route = ["fieldDeclarations", "arrayPositions"].map(|field| {
            self.value(
                input.reference,
                "dataforgeReferenceRoute",
                field,
                Conversion,
            )
        });
        let [declarations, positions] = route;
        self.put(
            &format!("{path}/field"),
            &[declarations?, positions?],
            complete,
        )?;
        let selector = self.value(input.reference, "dataforgeValue", "$value", Selector)?;
        let guid = if input.stored_guid {
            vec![self.value(input.reference, "dataforgeValue", "$value", Payload)?]
        } else {
            vec![
                selector.clone(),
                self.header(input.target_record, "guid", Conversion)?,
            ]
        };
        self.put(&format!("{path}/canvas_guid"), &guid, complete)?;
        self.canvas(&path, input.canvas, &selector, &guid, complete)?;
        let route = Route::new(&selector, input.canvas, complete);
        self.nested(&path, row, &input.assets, &route)
    }

    fn canvas(
        &mut self,
        path: &str,
        input: CanvasInput,
        selector: &SourceUse,
        guid: &[SourceUse],
        complete: bool,
    ) -> Result<()> {
        if let CanvasInput::Read {
            record_index,
            width_read,
            height_read,
            ..
        } = input
        {
            let file = self.header(record_index, "fileName", Conversion)?;
            self.put(
                &format!("{path}/canvas"),
                &[selector.clone(), file],
                complete,
            )?;
            self.dimension(
                path,
                "width",
                "size.x",
                (record_index, width_read),
                selector,
                complete,
            )?;
            self.dimension(
                path,
                "height",
                "size.y",
                (record_index, height_read),
                selector,
                complete,
            )?;
        } else {
            let fallback = if matches!(input, CanvasInput::Unreadable) {
                guid
            } else {
                slice::from_ref(selector)
            };
            self.put(&format!("{path}/canvas"), fallback, false)?;
            for axis in ["width", "height"] {
                self.put(&format!("{path}/{axis}"), slice::from_ref(selector), false)?;
            }
        }
        Ok(())
    }

    fn dimension(
        &mut self,
        path: &str,
        axis: &str,
        field: &str,
        (record_index, read): (u32, bool),
        selector: &SourceUse,
        complete: bool,
    ) -> Result<()> {
        let mut inputs = vec![selector.clone()];
        if read {
            inputs.push(self.scope.at(
                SourceLocation::DataForge {
                    record_index,
                    field: field.to_owned(),
                },
                Conversion,
            )?);
        }
        self.put(&format!("{path}/{axis}"), &inputs, complete && read)
    }

    fn movie(&mut self, index: usize, input: &MovieInput) -> Result<()> {
        let path = format!("/movies/{index}");
        self.put(&path, &[], false)?;
        self.put(&format!("{path}/port"), &[], input.is_root)?;
        self.entity(
            &format!("{path}/entity"),
            input.entity_record,
            input.is_root,
        )?;
        let movie = self.value(input.movie_path, "dataforgeValue", "$value", Payload)?;
        let complete = input.is_root && input.entity_record == Some(input.movie_path.record_index);
        self.put(&format!("{path}/movie"), &[movie], complete)
    }

    fn header(&self, record: u32, field: &str, role: SourceUseRole) -> Result<SourceUse> {
        self.observe(
            "dataforgeRecordHeader",
            vec![u64::from(record)],
            field,
            role,
        )
    }

    fn value(
        &self,
        value: ValueInput,
        namespace: &str,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        self.observe(
            namespace,
            vec![u64::from(value.record_index), value.ordinal as u64],
            field,
            role,
        )
    }

    fn observe(
        &self,
        namespace: &str,
        indices: Vec<u64>,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.scope.at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: field.to_owned(),
            },
            role,
        )?)
    }

    fn put(&mut self, path: &str, sources: &[SourceUse], complete: bool) -> Result<()> {
        Ok(self
            .fields
            .insert(FieldPath::parse(path)?, self.uses.origin(sources, complete))?)
    }
}

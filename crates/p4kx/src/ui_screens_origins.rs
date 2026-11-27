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

#[path = "ui_screens_attachment_origins.rs"]
mod attachments;

#[path = "ui_screens_canvas_routes.rs"]
mod canvas_routes;

#[path = "ui_screens_widget_origins.rs"]
mod widgets;

use self::attachments::AttachmentEvidence;
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
        let attachments = inputs
            .attachment_routes()
            .iter()
            .map(|route| projection.attachment(route))
            .collect::<Result<Vec<_>>>()?;
        projection.base(inputs.entity_record(), inputs.brand_style_record())?;
        for (index, (row, input)) in observed
            .listing()
            .screens
            .iter()
            .zip(inputs.screens())
            .enumerate()
        {
            let attachment = AttachmentEvidence::at(&attachments, input.attachment_route)?;
            projection.screen(index, row, input, attachment)?;
        }
        for (index, input) in inputs.movies().iter().enumerate() {
            let attachment = AttachmentEvidence::at(&attachments, input.attachment_route)?;
            projection.movie(index, input, attachment)?;
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

struct ScreenContext<'source> {
    selector: &'source SourceUse,
    known_owner: bool,
    attachment: &'source AttachmentEvidence,
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
        self.entity("/entity", record, AttachmentEvidence::root())?;
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

    fn entity(
        &mut self,
        path: &str,
        record: Option<u32>,
        attachment: &AttachmentEvidence,
    ) -> Result<()> {
        if let Some(index) = record {
            let name = self.header(index, "recordName", Conversion)?;
            self.put_attached(path, &[name], true, attachment)
        } else {
            self.put_attached(path, &[], false, attachment)
        }
    }

    fn screen(
        &mut self,
        index: usize,
        row: &Screen,
        input: &ScreenInput,
        attachment: &AttachmentEvidence,
    ) -> Result<()> {
        let path = format!("/screens/{index}");
        let known_owner = input.entity_record == Some(input.reference.record_index);
        self.put_attached(&path, &[], false, attachment)?;
        self.put_attached(&format!("{path}/port"), &[], true, attachment)?;
        self.entity(&format!("{path}/entity"), input.entity_record, attachment)?;
        let file = input
            .entity_record
            .map(|index| self.header(index, "fileName", Conversion))
            .transpose()?;
        self.put_attached(
            &format!("{path}/entity_record_path"),
            file.as_slice(),
            file.is_some(),
            attachment,
        )?;

        let [declarations, positions] = ["fieldDeclarations", "arrayPositions"].map(|field| {
            self.value(
                input.reference,
                "dataforgeReferenceRoute",
                field,
                Conversion,
            )
        });
        self.put_attached(
            &format!("{path}/field"),
            &[declarations?, positions?],
            known_owner,
            attachment,
        )?;
        let selector = self.value(input.reference, "dataforgeValue", "$value", Selector)?;
        let context = ScreenContext {
            selector: &selector,
            known_owner,
            attachment,
        };
        let guid = if input.stored_guid {
            vec![self.value(input.reference, "dataforgeValue", "$value", Payload)?]
        } else {
            vec![
                selector.clone(),
                self.header(input.target_record, "guid", Conversion)?,
            ]
        };
        self.put_attached(
            &format!("{path}/canvas_guid"),
            &guid,
            known_owner,
            attachment,
        )?;
        self.canvas(&path, input.canvas, &guid, &context)?;
        let canvases = self.canvas_routes(input.canvas, &input.assets.canvas_routes)?;
        let route = Route::new(&context, &canvases);
        self.nested(&path, row, &input.assets, &route)
    }

    fn canvas(
        &mut self,
        path: &str,
        input: CanvasInput,
        guid: &[SourceUse],
        context: &ScreenContext<'_>,
    ) -> Result<()> {
        let selector = context.selector;
        let attachment = context.attachment;
        if let CanvasInput::Read {
            record_index,
            width_read,
            height_read,
            ..
        } = input
        {
            let file = self.header(record_index, "fileName", Conversion)?;
            self.put_attached(
                &format!("{path}/canvas"),
                &[selector.clone(), file],
                context.known_owner,
                attachment,
            )?;
            self.dimension(path, "width", "size.x", (record_index, width_read), context)?;
            self.dimension(
                path,
                "height",
                "size.y",
                (record_index, height_read),
                context,
            )?;
        } else {
            let fallback = if matches!(input, CanvasInput::Unreadable) {
                guid
            } else {
                slice::from_ref(selector)
            };
            self.put_attached(&format!("{path}/canvas"), fallback, false, attachment)?;
            for axis in ["width", "height"] {
                self.put_attached(
                    &format!("{path}/{axis}"),
                    slice::from_ref(selector),
                    false,
                    attachment,
                )?;
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
        context: &ScreenContext<'_>,
    ) -> Result<()> {
        let mut inputs = vec![context.selector.clone()];
        if read {
            inputs.push(self.scope.at(
                SourceLocation::DataForge {
                    record_index,
                    field: field.to_owned(),
                },
                Conversion,
            )?);
        }
        self.put_attached(
            &format!("{path}/{axis}"),
            &inputs,
            context.known_owner && read,
            context.attachment,
        )
    }

    fn movie(
        &mut self,
        index: usize,
        input: &MovieInput,
        attachment: &AttachmentEvidence,
    ) -> Result<()> {
        let path = format!("/movies/{index}");
        self.put_attached(&path, &[], false, attachment)?;
        self.put_attached(&format!("{path}/port"), &[], true, attachment)?;
        self.entity(&format!("{path}/entity"), input.entity_record, attachment)?;
        let movie = self.value(input.movie_path, "dataforgeValue", "$value", Payload)?;
        let complete = input.entity_record == Some(input.movie_path.record_index);
        self.put_attached(&format!("{path}/movie"), &[movie], complete, attachment)
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

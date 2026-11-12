// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_extraction::{SourceUse, SourceUseRole};
use p4kx_screens::{CanvasInput, Screen, ScreenAssetInputs, WidgetAssetInput, WidgetInput};

use super::Projection;

pub(super) struct Route<'source> {
    root_selector: &'source SourceUse,
    root_record: Option<u32>,
    complete: bool,
}

impl<'source> Route<'source> {
    pub(super) const fn new(
        root_selector: &'source SourceUse,
        canvas: CanvasInput,
        complete: bool,
    ) -> Self {
        let root_record = match canvas {
            CanvasInput::Read { record_index, .. } => Some(record_index),
            _ => None,
        };
        Self {
            root_selector,
            root_record,
            complete,
        }
    }

    fn proves(&self, record: u32) -> bool {
        self.complete && self.root_record == Some(record)
    }
}

impl Projection {
    pub(super) fn nested(
        &mut self,
        path: &str,
        row: &Screen,
        source: &ScreenAssetInputs,
        route: &Route<'_>,
    ) -> Result<()> {
        self.put(&format!("{path}/assets"), &[], false)?;
        let canvases = format!("{path}/assets/canvases");
        self.put(&canvases, &[], false)?;
        ensure!(
            source.canvases.len() == row.assets.canvases.len(),
            "canvas origin count"
        );
        for (index, &record) in source.canvases.iter().enumerate() {
            let header = self.header(record, "fileName", SourceUseRole::Conversion)?;
            self.put(
                &format!("{canvases}/{index}"),
                &[route.root_selector.clone(), header],
                route.proves(record),
            )?;
        }
        for (name, count, inputs) in [
            ("textures", row.assets.textures.len(), &source.textures),
            ("vectors", row.assets.vectors.len(), &source.vectors),
            ("movies", row.assets.movies.len(), &source.movies),
            (
                "localisation_keys",
                row.assets.localisation_keys.len(),
                &source.localisation_keys,
            ),
        ] {
            ensure!(inputs.len() == count, "widget asset origin count");
            self.widget_values(&format!("{path}/assets/{name}"), inputs, route)?;
        }
        let path = format!("{path}/elements");
        self.put(&path, &[], false)?;
        ensure!(
            source.elements.len() == row.elements.len(),
            "widget group origin count"
        );
        for (index, (inputs, element)) in source.elements.iter().zip(&row.elements).enumerate() {
            ensure!(inputs.len() == element.count, "counted widget origin count");
            self.widget_types(&format!("{path}/{index}"), inputs, route)?;
        }
        Ok(())
    }

    fn widget_values(
        &mut self,
        path: &str,
        inputs: &[WidgetAssetInput],
        route: &Route<'_>,
    ) -> Result<()> {
        self.put(path, &[], false)?;
        for (index, input) in inputs.iter().enumerate() {
            let widget = input.widget;
            let value = self.observe(
                "dataforgeWidgetValue",
                vec![
                    u64::from(widget.record_index),
                    widget.slot as u64,
                    input.ordinal as u64,
                ],
                widget.section.field_name(),
                SourceUseRole::Conversion,
            )?;
            self.put(
                &format!("{path}/{index}"),
                &[route.root_selector.clone(), value],
                route.proves(widget.record_index),
            )?;
        }
        Ok(())
    }

    fn widget_types(
        &mut self,
        path: &str,
        inputs: &[WidgetInput],
        route: &Route<'_>,
    ) -> Result<()> {
        let Some((first, rest)) = inputs.split_first() else {
            anyhow::bail!("widget count has no source declarations");
        };
        let mut sources = vec![route.root_selector.clone(), self.widget_type(*first)?];
        let first_complete = route.proves(first.record_index);
        for name in ["element", "rendered"] {
            self.put(&format!("{path}/{name}"), &sources, first_complete)?;
        }
        for &widget in rest {
            sources.push(self.widget_type(widget)?);
        }
        let complete = inputs
            .iter()
            .all(|widget| route.proves(widget.record_index));
        self.put(&format!("{path}/count"), &sources, complete)?;
        self.put(path, &sources, complete)
    }

    fn widget_type(&self, widget: WidgetInput) -> Result<SourceUse> {
        self.observe(
            "dataforgeWidgetType",
            vec![u64::from(widget.record_index), widget.slot as u64],
            widget.section.field_name(),
            SourceUseRole::Conversion,
        )
    }
}

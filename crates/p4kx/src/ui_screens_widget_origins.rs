// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use anyhow::{Result, ensure};
use p4kx_extraction::{SourceUse, SourceUseRole};
use p4kx_screens::{Screen, ScreenAssetInputs, WidgetAssetInput, WidgetInput};

use super::canvas_routes::CanvasRoutes;
use super::{Projection, ScreenContext};

pub(super) struct Route<'source> {
    screen: &'source ScreenContext<'source>,
    canvases: &'source CanvasRoutes,
}

impl<'source> Route<'source> {
    pub(super) const fn new(
        screen: &'source ScreenContext<'source>,
        canvases: &'source CanvasRoutes,
    ) -> Self {
        Self { screen, canvases }
    }

    fn append(&self, record: u32, sources: &mut Vec<SourceUse>) -> bool {
        self.canvases.append(record, sources) && self.screen.known_owner
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
            let mut inputs = vec![route.screen.selector.clone(), header];
            let complete = route.append(record, &mut inputs);
            self.put_attached(
                &format!("{canvases}/{index}"),
                &inputs,
                complete,
                route.screen.attachment,
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
            let mut sources = vec![route.screen.selector.clone(), value];
            let complete = route.append(widget.record_index, &mut sources);
            self.put_attached(
                &format!("{path}/{index}"),
                &sources,
                complete,
                route.screen.attachment,
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
        let mut sources = vec![route.screen.selector.clone(), self.widget_type(*first)?];
        let mut complete = route.append(first.record_index, &mut sources);
        for name in ["element", "rendered"] {
            self.put_attached(
                &format!("{path}/{name}"),
                &sources,
                complete,
                route.screen.attachment,
            )?;
        }
        let mut seen = HashSet::from([first.record_index]);
        for &widget in rest {
            sources.push(self.widget_type(widget)?);
            if seen.insert(widget.record_index) {
                complete &= route.append(widget.record_index, &mut sources);
            }
        }
        self.put_attached(
            &format!("{path}/count"),
            &sources,
            complete,
            route.screen.attachment,
        )?;
        self.put_attached(path, &sources, complete, route.screen.attachment)
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

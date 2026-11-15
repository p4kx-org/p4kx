// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_gfx::Rgba;

use crate::colour::{ColorSpec, Palette};
use crate::images::Images;
use crate::paint::Painter;
use crate::report::{ContentOmissionCode, OmissionInput, Report};
use crate::screen_asset_inputs::WidgetInput;
use crate::still_inputs::TrackedInputs;
use crate::text::Texts;
use crate::widget::Widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum OmissionLocation {
    WidgetField(&'static str),
    Binding(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ContentOmissionKey {
    widget: WidgetInput,
    location: OmissionLocation,
    code: ContentOmissionCode,
}

#[derive(Debug, Default)]
pub(crate) struct Context<'t> {
    pub(crate) palette: Palette,
    pub(crate) painter: Painter,
    pub(crate) report: Report,
    pub(crate) content_omissions_seen: HashSet<ContentOmissionKey>,
    pub(crate) texts: Texts<'t>,
    pub(crate) images: Images<'t>,
    pub(crate) inputs: Option<TrackedInputs>,
    #[cfg(test)]
    pub(crate) power_trace: Option<crate::power_source_witness::Trace>,
}

impl Context<'_> {
    pub(crate) fn content_omission(
        &mut self,
        widget: &Widget,
        field: &str,
        source: Option<&str>,
        code: ContentOmissionCode,
        input: OmissionInput,
    ) {
        let location = match input {
            OmissionInput::WidgetField(name) => OmissionLocation::WidgetField(name),
            OmissionInput::Binding { operation, .. } => OmissionLocation::Binding(operation),
        };
        if self.content_omissions_seen.insert(ContentOmissionKey {
            widget: widget.input,
            location,
            code,
        }) {
            self.report
                .content_omission(widget, field, source, code, input);
        }
    }

    pub(crate) fn colour(&mut self, spec: &ColorSpec, alpha: f32, widget: &Widget) -> Option<Rgba> {
        match self.palette.resolve(spec, alpha) {
            Ok(colour) => Some(colour),
            Err(why) => {
                self.report.note(&widget.type_name, &why, &widget.name);
                None
            }
        }
    }
}

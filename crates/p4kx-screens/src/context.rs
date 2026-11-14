// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::Rgba;

use crate::colour::{ColorSpec, Palette};
use crate::images::Images;
use crate::paint::Painter;
use crate::report::Report;
use crate::still_inputs::TrackedInputs;
use crate::text::Texts;
use crate::widget::Widget;

#[derive(Debug, Default)]
pub(crate) struct Context<'t> {
    pub(crate) palette: Palette,
    pub(crate) painter: Painter,
    pub(crate) report: Report,
    pub(crate) texts: Texts<'t>,
    pub(crate) images: Images<'t>,
    pub(crate) inputs: Option<TrackedInputs>,
}

impl Context<'_> {
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

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::template_configuration::{MissionFlowConfiguration, MissionPropertyConfiguration};
use crate::template_projection::TemplateProjection;

/// Context rows computed from canonical physical Template owners.
#[derive(Debug, Default)]
pub(crate) struct TemplateConfigurations<'source> {
    pub(crate) templates: Vec<TemplateProjection<'source>>,
    pub(crate) flows: Vec<MissionFlowConfiguration>,
    pub(crate) properties: Vec<MissionPropertyConfiguration>,
}

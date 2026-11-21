// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::cryxml::SourceElementPosition;
use p4kx_extraction::{
    FieldOrigins, FieldPath, SourceInventory, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use super::label::{LabelOrigin, LabelUse};
use crate::action_map_profile::{ActionMapProfile, AttributeSite};
use crate::action_map_source::{attribute_location, node_location, scope};
use crate::action_map_write::ActionMapLabels;

/// Retains the sources that determine the counts in an action-map export summary.
///
/// # Errors
/// Returns an error if the source inventory or field evidence cannot be retained.
pub fn action_map_summary_origins(
    profile: &ActionMapProfile,
    labels: &ActionMapLabels<'_>,
) -> io::Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let (source, profile_complete) = scope(&uses, profile.origin.as_ref())?;
    let labels = LabelOrigin::new(&uses, labels.entry(), labels.texts())?;
    let count = CountOrigin {
        source: &source,
        labels: &labels,
    };
    let root = count.survey(&profile.root, "actionMapSelection")?;
    let mut maps = vec![root.clone()];
    let mut actions = vec![root.clone()];
    let mut unresolved = vec![root];
    let mut complete = profile_complete;
    for (map, site) in profile.maps().maps.iter().zip(&profile.sites) {
        let name = count.attribute(&site.name)?;
        maps.push(name.clone());
        actions.push(name.clone());
        unresolved.push(name);
        let survey = count.survey(&site.node, "actionSelection")?;
        actions.push(survey.clone());
        unresolved.push(survey);
        complete &= count.label(
            map.label_key.as_deref(),
            site.label.as_ref(),
            &site.node,
            &mut unresolved,
        )?;
        for (action, site) in map.actions.iter().zip(&site.actions) {
            let name = count.attribute(&site.name)?;
            actions.push(name.clone());
            unresolved.push(name);
            complete &= count.label(
                action.label_key.as_deref(),
                site.label.as_ref(),
                &site.node,
                &mut unresolved,
            )?;
        }
    }
    let mut fields = FieldOrigins::new();
    for (path, inputs, complete) in [
        ("/archive", [].as_slice(), true),
        ("/out", [].as_slice(), true),
        ("/format", [].as_slice(), true),
        ("/language", [].as_slice(), true),
        ("/origin_receipt", [].as_slice(), true),
        ("/action_maps", maps.as_slice(), profile_complete),
        ("/actions", actions.as_slice(), profile_complete),
        ("/unresolved_labels", unresolved.as_slice(), complete),
    ] {
        fields
            .insert(
                FieldPath::parse(path).map_err(io::Error::other)?,
                uses.origin(inputs, complete),
            )
            .map_err(io::Error::other)?;
    }
    Ok((fields, uses.freeze().map_err(io::Error::other)?))
}

struct CountOrigin<'input> {
    source: &'input SourceScope,
    labels: &'input LabelOrigin<'input>,
}

impl CountOrigin<'_> {
    fn survey(&self, node: &SourceElementPosition, field: &str) -> io::Result<SourceUse> {
        self.source
            .at(node_location(node, field)?, SourceUseRole::Selector)
            .map_err(io::Error::other)
    }

    fn attribute(&self, site: &AttributeSite) -> io::Result<SourceUse> {
        self.source
            .at(
                attribute_location(&site.position, "value")?,
                SourceUseRole::Selector,
            )
            .map_err(io::Error::other)
    }

    fn label(
        &self,
        key: Option<&str>,
        site: Option<&AttributeSite>,
        node: &SourceElementPosition,
        inputs: &mut Vec<SourceUse>,
    ) -> io::Result<bool> {
        let Some((key, site)) = key.zip(site) else {
            inputs.push(self.survey(node, "labelKeySelection")?);
            return Ok(true);
        };
        let mut label = vec![self.attribute(site)?];
        let complete = self
            .labels
            .contribute(key, &mut label, LabelUse::ResolutionCount)?;
        inputs.append(&mut label);
        Ok(complete)
    }
}

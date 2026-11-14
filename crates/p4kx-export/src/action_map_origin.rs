// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::cryxml::SourceElementPosition;
use p4kx_extraction::{
    DecimalU64, FieldOrigin, FieldPath, LocalizationEntry, OriginArtifactBinding,
    OriginOutputAddress, SourceInventory, SourceScope, SourceUse, SourceUseRole, SourceUses,
    TextSet, write_origin_companion,
};

use self::label::{LabelOrigin, LabelUse};
pub use self::summary::action_map_summary_origins;
use crate::action_map::{Action, Device};
use crate::action_map_profile::{
    ActionMapProfile, ActionSites, AttributeSite, BindingSites, DeviceSite,
};
use crate::action_map_source::{attribute_location, node_location, scope};

#[path = "action_map_label_origin.rs"]
mod label;
#[path = "action_map_summary_origins.rs"]
mod summary;

/// The exact output fields and original sources of an action-map export.
#[derive(Debug)]
pub struct ActionMapOrigins {
    inventory: SourceInventory,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl ActionMapOrigins {
    /// Writes the companion for the actual published bytes.
    ///
    /// # Errors
    /// Returns an error if the origin stream cannot be written.
    pub fn write(&self, out: &mut dyn Write, binding: &OriginArtifactBinding) -> io::Result<()> {
        write_origin_companion(
            out,
            binding,
            &self.inventory,
            self.fields
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

pub(crate) struct Recorder<'input> {
    uses: SourceUses,
    profile: &'input ActionMapProfile,
    profile_source: SourceScope,
    profile_complete: bool,
    labels: LabelOrigin<'input>,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl<'input> Recorder<'input> {
    pub(crate) fn new(
        profile: &'input ActionMapProfile,
        locale: &'input LocalizationEntry,
        texts: &'input TextSet<'input>,
    ) -> io::Result<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let (profile_source, profile_complete) = scope(&uses, profile.origin.as_ref())?;
        let labels = LabelOrigin::new(&uses, locale, texts)?;
        Ok(Self {
            uses,
            profile,
            profile_source,
            profile_complete,
            labels,
            fields: Vec::new(),
        })
    }

    pub(crate) fn finish(self) -> io::Result<ActionMapOrigins> {
        Ok(ActionMapOrigins {
            inventory: self.uses.freeze().map_err(io::Error::other)?,
            fields: self.fields,
        })
    }

    pub(crate) fn json(&mut self) -> io::Result<()> {
        self.json_field("/language", self.uses.origin(&[], true))?;
        self.json_field(
            "/action_maps",
            self.node_field(&self.profile.root, "actionMapSelection")?,
        )?;
        for (map_index, (map, sites)) in self
            .profile
            .maps()
            .maps
            .iter()
            .zip(&self.profile.sites)
            .enumerate()
        {
            let path = format!("/action_maps/{map_index}");
            self.json_field(&format!("{path}/name"), self.attribute_field(&sites.name)?)?;
            self.json_field(
                &format!("{path}/label_key"),
                self.optional_attribute(sites.label.as_ref(), &sites.node, "labelKeySelection")?,
            )?;
            self.json_field(
                &format!("{path}/label"),
                self.label(map.label_key.as_deref(), sites.label.as_ref(), &sites.node)?,
            )?;
            self.json_field(
                &format!("{path}/actions"),
                self.node_field(&sites.node, "actionSelection")?,
            )?;
            for (action_index, (action, action_sites)) in
                map.actions.iter().zip(&sites.actions).enumerate()
            {
                self.json_action(
                    &format!("{path}/actions/{action_index}"),
                    action.label_key.as_deref(),
                    action_sites,
                )?;
            }
        }
        Ok(())
    }

    fn json_action(
        &mut self,
        path: &str,
        label_key: Option<&str>,
        sites: &ActionSites,
    ) -> io::Result<()> {
        self.json_field(&format!("{path}/name"), self.attribute_field(&sites.name)?)?;
        self.json_field(
            &format!("{path}/label_key"),
            self.optional_attribute(sites.label.as_ref(), &sites.node, "labelKeySelection")?,
        )?;
        self.json_field(
            &format!("{path}/label"),
            self.label(label_key, sites.label.as_ref(), &sites.node)?,
        )?;
        self.json_field(
            &format!("{path}/activation_mode"),
            self.optional_attribute(sites.mode.as_ref(), &sites.node, "activationModeSelection")?,
        )?;
        self.json_field(
            &format!("{path}/bindings"),
            self.node_field(&sites.node, "bindingSelection")?,
        )?;
        for (binding_index, binding) in sites.bindings.iter().enumerate() {
            let path = format!("{path}/bindings/{binding_index}");
            self.json_field(
                &format!("{path}/device"),
                self.uses
                    .origin(&[self.device(binding)?], self.profile_complete),
            )?;
            self.json_field(
                &format!("{path}/input"),
                self.attribute_field(&binding.input)?,
            )?;
            self.json_field(
                &format!("{path}/activation_mode"),
                self.binding_mode(binding, &sites.node)?,
            )?;
        }
        Ok(())
    }

    pub(crate) fn csv(
        &mut self,
        row: &CsvRow,
        column: CsvColumn,
        range: std::ops::Range<u64>,
    ) -> io::Result<()> {
        let (Some(map), Some(sites)) = (
            self.profile.maps().maps.get(row.map),
            self.profile.sites.get(row.map),
        ) else {
            return Err(io::Error::other("action-map row has no original map"));
        };
        let (Some(action), Some(action_sites)) =
            (map.actions.get(row.action), sites.actions.get(row.action))
        else {
            return Err(io::Error::other("action-map row has no original action"));
        };
        let (column_name, origin) = match column {
            CsvColumn::Map => ("action_map", self.attribute_field(&sites.name)?),
            CsvColumn::MapLabel => (
                "action_map_label",
                self.label(map.label_key.as_deref(), sites.label.as_ref(), &sites.node)?,
            ),
            CsvColumn::Action => ("action", self.attribute_field(&action_sites.name)?),
            CsvColumn::ActionLabel => (
                "label",
                self.label(
                    action.label_key.as_deref(),
                    action_sites.label.as_ref(),
                    &action_sites.node,
                )?,
            ),
            CsvColumn::ActivationMode => (
                "activation_mode",
                self.optional_attribute(
                    action_sites.mode.as_ref(),
                    &action_sites.node,
                    "activationModeSelection",
                )?,
            ),
            CsvColumn::Device(device) => (
                device.name(),
                self.device_cell(action, action_sites, device)?,
            ),
        };
        let field = FieldPath::parse(&format!("/rows/{}/{column_name}", row.index))
            .map_err(io::Error::other)?;
        self.fields.try_reserve(1).map_err(io::Error::other)?;
        self.fields.push((
            OriginOutputAddress::TextBytes {
                start: DecimalU64::new(range.start),
                end_exclusive: DecimalU64::new(range.end),
                field,
            },
            origin,
        ));
        Ok(())
    }

    fn device_cell(
        &self,
        action: &Action,
        sites: &ActionSites,
        device: Device,
    ) -> io::Result<FieldOrigin> {
        let mut inputs = Vec::new();
        for (binding, site) in action.bindings.iter().zip(&sites.bindings) {
            if binding.device == device {
                self.add_attribute(&mut inputs, &site.input, SourceUseRole::Payload)?;
                inputs.push(self.device(site)?);
                let role = if binding.activation_mode == action.activation_mode {
                    SourceUseRole::Selector
                } else {
                    SourceUseRole::Payload
                };
                self.add_mode(&mut inputs, site, &sites.node, role)?;
                if let Some(mode) = &sites.mode {
                    self.add_attribute(&mut inputs, mode, SourceUseRole::Selector)?;
                }
            }
        }
        if inputs.is_empty() {
            inputs.push(self.node(&sites.node, device.name())?);
        }
        Ok(self.uses.origin(&inputs, self.profile_complete))
    }

    fn json_field(&mut self, path: &str, origin: FieldOrigin) -> io::Result<()> {
        let path = FieldPath::parse(path).map_err(io::Error::other)?;
        self.fields.try_reserve(1).map_err(io::Error::other)?;
        self.fields
            .push((OriginOutputAddress::JsonField { path }, origin));
        Ok(())
    }

    fn label(
        &self,
        key: Option<&str>,
        selector: Option<&AttributeSite>,
        node: &SourceElementPosition,
    ) -> io::Result<FieldOrigin> {
        let Some((key, selector)) = key.zip(selector) else {
            return self.node_field(node, "labelKeySelection");
        };
        let mut inputs = Vec::new();
        self.add_attribute(&mut inputs, selector, SourceUseRole::Selector)?;
        let complete = self.labels.contribute(key, &mut inputs, LabelUse::Output)?;
        Ok(self.uses.origin(&inputs, self.profile_complete && complete))
    }

    fn attribute_field(&self, site: &AttributeSite) -> io::Result<FieldOrigin> {
        let mut inputs = Vec::new();
        self.add_attribute(&mut inputs, site, SourceUseRole::Payload)?;
        Ok(self.uses.origin(&inputs, self.profile_complete))
    }

    fn optional_attribute(
        &self,
        site: Option<&AttributeSite>,
        node: &SourceElementPosition,
        field: &str,
    ) -> io::Result<FieldOrigin> {
        site.map_or_else(
            || self.node_field(node, field),
            |site| self.attribute_field(site),
        )
    }

    fn add_attribute(
        &self,
        inputs: &mut Vec<SourceUse>,
        site: &AttributeSite,
        role: SourceUseRole,
    ) -> io::Result<()> {
        let location = attribute_location(&site.position, "value")?;
        inputs.push(
            self.profile_source
                .at(location.clone(), role)
                .map_err(io::Error::other)?,
        );
        if site.converted {
            inputs.push(
                self.profile_source
                    .at(location, SourceUseRole::Conversion)
                    .map_err(io::Error::other)?,
            );
        }
        Ok(())
    }

    fn node_field(&self, node: &SourceElementPosition, field: &str) -> io::Result<FieldOrigin> {
        Ok(self
            .uses
            .origin(&[self.node(node, field)?], self.profile_complete))
    }

    fn node(&self, node: &SourceElementPosition, field: &str) -> io::Result<SourceUse> {
        self.profile_source
            .at(node_location(node, field)?, SourceUseRole::Selector)
            .map_err(io::Error::other)
    }

    fn device(&self, binding: &BindingSites) -> io::Result<SourceUse> {
        let location = match &binding.device {
            DeviceSite::Attribute => attribute_location(&binding.input.position, "name")?,
            DeviceSite::Element(node) => node_location(node, "name")?,
        };
        self.profile_source
            .at(location, SourceUseRole::Conversion)
            .map_err(io::Error::other)
    }

    fn binding_mode(
        &self,
        binding: &BindingSites,
        action: &SourceElementPosition,
    ) -> io::Result<FieldOrigin> {
        let mut inputs = Vec::new();
        self.add_mode(&mut inputs, binding, action, SourceUseRole::Payload)?;
        Ok(self.uses.origin(&inputs, self.profile_complete))
    }

    fn add_mode(
        &self,
        inputs: &mut Vec<SourceUse>,
        binding: &BindingSites,
        action: &SourceElementPosition,
        role: SourceUseRole,
    ) -> io::Result<()> {
        if let Some(mode) = &binding.mode {
            self.add_attribute(inputs, mode, role)?;
        } else {
            inputs.push(self.node(action, "activationModeSelection")?);
        }
        if binding.inherited
            && let DeviceSite::Element(node) = &binding.device
        {
            inputs.push(self.node(node, "activationModeSelection")?);
        }
        Ok(())
    }
}

pub(crate) struct CsvRow {
    pub(crate) map: usize,
    pub(crate) action: usize,
    pub(crate) index: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum CsvColumn {
    Map,
    MapLabel,
    Action,
    ActionLabel,
    ActivationMode,
    Device(Device),
}

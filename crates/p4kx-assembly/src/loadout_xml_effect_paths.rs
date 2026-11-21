// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Node, Scope};
use crate::resolver_observation::Observation;
use crate::{LoadoutPath, LoadoutPathOperation};

impl Scope<'_, '_> {
    pub(crate) fn loadout_port_name(
        &self,
        node: Node<'_, '_>,
        name: &str,
    ) -> Result<Option<LoadoutPath>, String> {
        if self.source.is_some()
            && self
                .observation
                .is_some_and(Observation::observes_xml_port_replacements)
        {
            self.path_attribute(node, name, LoadoutPathOperation::PortName)
        } else {
            self.port_name(node, name)
        }
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::Iter;

use crate::{Attachment, PortNode};

enum Step<'node> {
    Ports(Iter<'node, PortNode>),
    Ascend(usize),
}

pub(crate) fn visit<'node>(
    ports: &'node [PortNode],
    mut visit: impl FnMut(&str, &[&'node PortNode]),
) {
    let mut pending = vec![Step::Ports(ports.iter())];
    let mut path = String::new();
    let mut route = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Ports(mut ports) => {
                let Some(port) = ports.next() else { continue };
                pending.push(Step::Ports(ports));
                pending.push(Step::Ascend(path.len()));
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(port.name());
                route.push(port);
                visit(&path, &route);
                if let Attachment::Entity(node) = port.attachment()
                    && !node.ports().is_empty()
                {
                    pending.push(Step::Ports(node.ports().iter()));
                }
                if !port.nested().is_empty() {
                    pending.push(Step::Ports(port.nested().iter()));
                }
            }
            Step::Ascend(length) => {
                path.truncate(length);
                route.pop();
            }
        }
    }
}

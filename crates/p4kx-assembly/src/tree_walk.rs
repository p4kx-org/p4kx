// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::iter::Enumerate;
use std::slice::Iter;

use crate::{Attachment, PortNode};

enum Step<'node> {
    Ports(Enumerate<Iter<'node, PortNode>>),
    Ascend(usize),
}

pub(crate) fn visit<'node>(
    ports: &'node [PortNode],
    mut visit: impl FnMut(&str, &[&'node PortNode]),
) {
    match try_visit(ports, |path, route| {
        visit(path, route);
        Ok::<_, Infallible>(())
    }) {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

pub(crate) fn try_visit<'node, E>(
    ports: &'node [PortNode],
    mut visit: impl FnMut(&str, &[&'node PortNode]) -> Result<(), E>,
) -> Result<(), E> {
    try_visit_indexed(ports, |path, route, _| visit(path, route))
}

pub(crate) fn try_visit_indexed<'node, E>(
    ports: &'node [PortNode],
    mut visit: impl FnMut(&str, &[&'node PortNode], &[usize]) -> Result<(), E>,
) -> Result<(), E> {
    let mut pending = vec![Step::Ports(ports.iter().enumerate())];
    let mut path = String::new();
    let mut route = Vec::new();
    let mut ordinals = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Ports(mut ports) => {
                let Some((ordinal, port)) = ports.next() else {
                    continue;
                };
                pending.push(Step::Ports(ports));
                pending.push(Step::Ascend(path.len()));
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(port.name());
                route.push(port);
                ordinals.push(ordinal);
                visit(&path, &route, &ordinals)?;
                if let Attachment::Entity(node) = port.attachment()
                    && !node.ports().is_empty()
                {
                    pending.push(Step::Ports(node.ports().iter().enumerate()));
                }
                if !port.nested().is_empty() {
                    pending.push(Step::Ports(port.nested().iter().enumerate()));
                }
            }
            Step::Ascend(length) => {
                path.truncate(length);
                route.pop();
                ordinals.pop();
            }
        }
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod read;
mod walk;

use std::collections::HashMap;
use std::convert::Infallible;

use crate::DataForge;
use crate::tags::{Node, TagBuildEvent, TagTreeIssues};

use read::ReadTags;
use walk::Walk;

/// The tag tree before its GUID index is constructed.
pub(crate) struct Layout {
    pub(crate) nodes: Vec<Node>,
    pub(crate) roots: Vec<u32>,
    pub(crate) issues: TagTreeIssues,
}

pub(crate) fn layout(forge: &DataForge<'_>, stop: &dyn Fn() -> bool) -> Option<Layout> {
    match layout_observed(forge, stop, &mut |_| Ok::<(), Infallible>(())) {
        Ok(layout) => layout,
        Err(never) => match never {},
    }
}

pub(crate) fn layout_observed<'f, E>(
    forge: &'f DataForge<'f>,
    stop: &dyn Fn() -> bool,
    observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
) -> Result<Option<Layout>, E> {
    let mut ticker = Ticker::new(stop);
    let mut issues = TagTreeIssues::default();
    let Some(ReadTags { raws, roots }) = read::read(forge, &mut issues, &mut ticker, observe)?
    else {
        observe(TagBuildEvent::Cancelled)?;
        return Ok(None);
    };
    let mut positions = HashMap::with_capacity(raws.len());
    for (index, raw) in raws.iter().enumerate() {
        if ticker.stopped() {
            observe(TagBuildEvent::Cancelled)?;
            return Ok(None);
        }
        positions.insert(raw.guid, index);
    }
    let mut walk = Walk {
        raws: &raws,
        positions: &positions,
        visited: vec![false; raws.len()],
        nodes: Vec::with_capacity(raws.len()),
        next_node: 0,
        roots: Vec::new(),
        names: HashMap::new(),
        issues,
        ticker,
    };
    walk.listed_roots(roots, observe)?;
    walk.unlisted(observe)?;
    if walk.ticker.poll() {
        observe(TagBuildEvent::Cancelled)?;
        return Ok(None);
    }
    let Walk {
        nodes,
        roots,
        mut issues,
        ..
    } = walk;
    issues.unlisted = nodes.iter().filter(|node| !node.listed).count();
    Ok(Some(Layout {
        nodes,
        roots,
        issues,
    }))
}

const POLL_EVERY: u32 = 1024;

struct Ticker<'s> {
    stop: &'s dyn Fn() -> bool,
    steps: u32,
    stopped: bool,
}

impl<'s> Ticker<'s> {
    fn new(stop: &'s dyn Fn() -> bool) -> Self {
        Self {
            stop,
            steps: 0,
            stopped: stop(),
        }
    }

    fn stopped(&mut self) -> bool {
        self.steps += 1;
        if !self.stopped && self.steps >= POLL_EVERY {
            return self.poll();
        }
        self.stopped
    }

    fn poll(&mut self) -> bool {
        self.steps = 0;
        self.stopped = self.stopped || (self.stop)();
        self.stopped
    }
}

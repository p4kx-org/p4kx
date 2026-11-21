// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::Arc;

use super::{XmlInclude, XmlRoute};
use crate::{LoadoutDocumentKind, LoadoutFileObservation, LoadoutFileSource, LoadoutPath};

#[test]
fn shared_include_routes_can_be_formatted_and_dropped_on_a_small_stack() -> std::io::Result<()> {
    let worker = std::thread::Builder::new()
        .stack_size(64 << 10)
        .spawn(verify_shared_route)?;
    worker
        .join()
        .map_err(|_| std::io::Error::other("the shared Include route worker panicked"))?
        .map_err(std::io::Error::other)
}

fn verify_shared_route() -> fmt::Result {
    const LINKS: usize = 16_384;
    let path = LoadoutPath::unbound("synthetic/route.xml");
    let source = LoadoutFileObservation::new(
        LoadoutFileSource::unbound(&path),
        LoadoutDocumentKind::XmlLoadout,
        Ok(()),
    );
    let edge = Arc::new(XmlInclude {
        parent: source.clone(),
        child: source,
        path,
    });
    let route = |tail| {
        Arc::new(XmlRoute {
            edge: Arc::clone(&edge),
            tail,
        })
    };
    let mut shared = route(None);
    for _ in 1..LINKS {
        shared = route(Some(shared));
    }
    let watch = Arc::downgrade(&shared);
    let first = route(Some(Arc::clone(&shared)));
    let second = route(Some(shared));
    drop(first);
    assert!(
        watch.upgrade().is_some(),
        "the other route retains the shared tail"
    );

    fmt::write(&mut Sink, format_args!("{second:?}"))?;
    drop(second);
    assert!(
        watch.upgrade().is_none(),
        "the final owner releases the full route"
    );
    Ok(())
}

struct Sink;
impl fmt::Write for Sink {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }
}

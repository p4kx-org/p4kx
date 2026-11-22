// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::Arc;

use super::{XmlEntries, XmlLoadout};
use crate::loadout_file_trace::FileTrace;
use crate::loadout_xml_use::{XmlInclude, XmlRoute};
use crate::merge::PortMergeIndex;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::spec::EntrySpec;
use crate::{
    LoadoutError, LoadoutFileObservation, LoadoutFiles, LoadoutPath, LoadoutRequest,
    XmlPortReplacement,
};

type EntriesResult = Result<Arc<[EntrySpec]>, LoadoutError>;
type ReplacementCapture<'a> = (&'a LoadoutFileObservation, &'a mut Vec<XmlPortReplacement>);

pub(crate) fn load_observed(
    files: &dyn LoadoutFiles,
    path: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    include_read_budget: Option<usize>,
    trace: &mut FileTrace,
) -> Result<XmlEntries, LoadoutError> {
    let root = trace.position();
    let mut loader = Loader {
        observation,
        files,
        trace,
        active: HashSet::new(),
        done: HashMap::new(),
        include_read_budget,
        include_reads: 0,
        issues: Vec::new(),
        replacements: observation
            .is_some_and(Observation::observes_xml_port_replacements)
            .then(Vec::new),
    };
    let entries = loader.load(path, caller);
    loader.trace.set_xml(loader.trace.captured(root));
    entries.map(|entries| XmlEntries {
        entries,
        issues: loader.issues,
        replacements: loader.replacements.map(Arc::from),
    })
}

#[derive(Clone)]
struct Completed {
    result: EntriesResult,
    operation: Option<usize>,
}

enum Started {
    Pending(Frame),
    Complete(Completed),
}

struct Frame {
    path: LoadoutPath,
    key: String,
    operation: Option<usize>,
    includes: std::vec::IntoIter<LoadoutPath>,
    named: HashSet<String>,
    own: Vec<EntrySpec>,
    merged: Vec<EntrySpec>,
    ports: PortMergeIndex,
}

impl Frame {
    fn new(path: LoadoutPath, key: String, operation: Option<usize>, loadout: XmlLoadout) -> Self {
        Self {
            path,
            key,
            operation,
            includes: loadout.includes.into_iter(),
            named: HashSet::new(),
            own: loadout.entries,
            merged: Vec::new(),
            ports: PortMergeIndex::default(),
        }
    }

    fn next_include(
        &mut self,
        checkpoint: impl Fn() -> Result<(), LoadoutError>,
    ) -> Result<Option<(LoadoutPath, String)>, LoadoutError> {
        for path in self.includes.by_ref() {
            checkpoint()?;
            let key = path.to_ascii_lowercase();
            if self.named.insert(key.clone()) {
                return Ok(Some((path, key)));
            }
        }
        Ok(None)
    }

    fn merge(
        &mut self,
        entries: impl IntoIterator<Item = EntrySpec>,
        capture: Option<ReplacementCapture<'_>>,
    ) {
        if let Some((operation, replacements)) = capture {
            let document = &self.path;
            self.ports.merge_observed(
                &mut self.merged,
                entries,
                |entry| &entry.port,
                |ordinal, displaced, winner| {
                    replacements.push(XmlPortReplacement::new(
                        document, operation, ordinal, displaced, winner,
                    ));
                },
            );
        } else {
            self.ports
                .merge(&mut self.merged, entries, |entry| &entry.port);
        }
    }

    fn finish(&mut self, capture: Option<ReplacementCapture<'_>>) -> Arc<[EntrySpec]> {
        let own = mem::take(&mut self.own);
        self.merge(own, capture);
        Arc::from(mem::take(&mut self.merged))
    }
}

struct Loader<'load> {
    observation: Option<&'load Observation<'load>>,
    files: &'load dyn LoadoutFiles,
    trace: &'load mut FileTrace,
    active: HashSet<String>,
    done: HashMap<String, Completed>,
    include_read_budget: Option<usize>,
    include_reads: usize,
    issues: Vec<LoadoutError>,
    replacements: Option<Vec<XmlPortReplacement>>,
}

impl Loader<'_> {
    fn load(&mut self, path: &LoadoutPath, caller: &LoadoutRequest) -> EntriesResult {
        let mut current = match self.start(path, path.to_ascii_lowercase(), caller)? {
            Started::Pending(frame) => frame,
            Started::Complete(done) => return done.result,
        };
        let mut parents = Vec::new();
        loop {
            self.checkpoint()?;
            if let Some((path, key)) = current.next_include(|| self.checkpoint())? {
                let caller = LoadoutRequest::Xml { path: path.clone() };
                match self.start(&path, key, &caller)? {
                    Started::Pending(frame) => parents.push(mem::replace(&mut current, frame)),
                    Started::Complete(done) => {
                        if let Err(error) =
                            self.include(&mut current, &path, &caller, done, &parents)
                        {
                            return self.refuse(current, parents, error);
                        }
                    }
                }
                continue;
            }
            let capture = self.replacement_capture(current.operation)?;
            let entries = current.finish(capture);
            let path = current.path.clone();
            let complete = self.finish(current, Ok(entries));
            let Some(mut parent) = parents.pop() else {
                return complete.result;
            };
            let caller = LoadoutRequest::Xml { path: path.clone() };
            self.include(&mut parent, &path, &caller, complete, &parents)?;
            current = parent;
        }
    }

    fn start(
        &mut self,
        path: &LoadoutPath,
        key: String,
        caller: &LoadoutRequest,
    ) -> Result<Started, LoadoutError> {
        self.checkpoint()?;
        if let Some(done) = self.done.get(&key) {
            return Ok(Started::Complete(done.clone()));
        }
        if self.active.contains(&key) {
            return Ok(unread(LoadoutError::XmlLoadout {
                path: path.to_string(),
                reason: "it includes itself".to_owned(),
            }));
        }
        if let Err(error) = self.admit_include(path) {
            self.checkpoint()?;
            return Ok(unread(error));
        }
        let first = self.trace.position();
        let result = super::load_file(self.files, path, caller, self.observation, self.trace);
        self.checkpoint()?;
        let operation = self.trace.captured(first);
        match result {
            Ok(loadout) => {
                self.active.insert(key.clone());
                Ok(Started::Pending(Frame::new(
                    path.clone(),
                    key,
                    operation,
                    loadout,
                )))
            }
            Err(error) => Ok(Started::Complete(self.cache(key, operation, Err(error)))),
        }
    }

    fn admit_include(&mut self, path: &LoadoutPath) -> Result<(), LoadoutError> {
        if let Some(limit) = self.include_read_budget
            && !self.active.is_empty()
        {
            if self.include_reads >= limit {
                resolver_observation::notify(
                    self.observation,
                    LoadoutReadEvent::InventoryIncomplete,
                )?;
                return Err(LoadoutError::XmlIncludeReadBudgetExceeded {
                    path: path.to_string(),
                    limit,
                });
            }
            self.include_reads += 1;
        }
        Ok(())
    }

    fn include(
        &mut self,
        parent: &mut Frame,
        request: &LoadoutPath,
        caller: &LoadoutRequest,
        completed: Completed,
        ancestors: &[Frame],
    ) -> Result<(), LoadoutError> {
        self.checkpoint()?;
        let edge = if completed.operation.is_some() {
            self.trace.dependency(
                parent.operation,
                completed.operation,
                caller,
                self.observation,
            )?
        } else {
            if let Err(error) = &completed.result
                && !matches!(error, LoadoutError::ObservationStopped)
            {
                self.trace
                    .refused(parent.operation, caller, error, self.observation)?;
            }
            None
        };
        match completed.result {
            Ok(entries) => {
                let capture = self.replacement_capture(parent.operation)?;
                parent.merge(through_include(&entries, edge), capture);
            }
            Err(error @ LoadoutError::XmlIncludeReadBudgetExceeded { .. }) => return Err(error),
            Err(mut error) => {
                if let LoadoutError::MissingXmlLoadout { path } = &mut error
                    && self.observation.is_some()
                {
                    let includes: Vec<_> = ancestors
                        .iter()
                        .chain(std::iter::once(&*parent))
                        .skip(1)
                        .map(|frame| frame.path.clone())
                        .chain(std::iter::once(request.clone()))
                        .collect();
                    path.selected_by(&includes);
                }
                self.issues.push(error);
            }
        }
        Ok(())
    }

    fn replacement_capture(
        &mut self,
        position: Option<usize>,
    ) -> Result<Option<ReplacementCapture<'_>>, LoadoutError> {
        let Some(replacements) = self.replacements.as_mut() else {
            return Ok(None);
        };
        let Some(operation) = position.and_then(|position| self.trace.xml_operation(position))
        else {
            if let Some(observation) = self.observation {
                observation.stop();
            }
            return Err(LoadoutError::ObservationStopped);
        };
        Ok(Some((operation, replacements)))
    }

    fn finish(&mut self, frame: Frame, result: EntriesResult) -> Completed {
        self.active.remove(&frame.key);
        self.cache(frame.key, frame.operation, result)
    }

    fn cache(&mut self, key: String, operation: Option<usize>, result: EntriesResult) -> Completed {
        let completed = Completed { result, operation };
        self.done.insert(key, completed.clone());
        completed
    }

    fn refuse(
        &mut self,
        mut current: Frame,
        mut parents: Vec<Frame>,
        error: LoadoutError,
    ) -> EntriesResult {
        if !matches!(error, LoadoutError::XmlIncludeReadBudgetExceeded { .. }) {
            return Err(error);
        }
        loop {
            self.checkpoint()?;
            let caller = LoadoutRequest::Xml {
                path: current.path.clone(),
            };
            let completed = self.finish(current, Err(error.clone()));
            let Some(parent) = parents.pop() else {
                return completed.result;
            };
            self.trace.dependency(
                parent.operation,
                completed.operation,
                &caller,
                self.observation,
            )?;
            current = parent;
        }
    }

    fn checkpoint(&self) -> Result<(), LoadoutError> {
        if self.observation.is_some_and(Observation::stopped) {
            return Err(LoadoutError::ObservationStopped);
        }
        Ok(())
    }
}

const fn unread(error: LoadoutError) -> Started {
    Started::Complete(Completed {
        result: Err(error),
        operation: None,
    })
}

fn through_include(
    entries: &[EntrySpec],
    edge: Option<XmlInclude>,
) -> impl Iterator<Item = EntrySpec> {
    let edge = edge.map(Arc::new);
    let mut routes = HashMap::new();
    entries.iter().cloned().map(move |mut entry| {
        if let Some(edge) = &edge {
            entry.xml_route = Some(XmlRoute::prepend(entry.xml_route.take(), edge, &mut routes));
        }
        entry
    })
}

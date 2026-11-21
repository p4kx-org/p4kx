// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{AnimationChoice, ArchivePositionedAnimation, EntryOrigin, SourceLocation};
use p4kx_model::{Animation, Model};

use crate::cdf::{self, MAX_CDF_BYTES, is_character_definition};
use crate::gear_params::{self, ParameterSource};
use crate::gear_pose;
use crate::{ArchiveModels, DeployedGear, GearPoseSources, ModelFailure, ModelProvider};

pub(crate) use crate::gear_params::{
    ChrParams, MAX_CHRPARAMS_BYTES, MAX_PARAM_SOURCES, read_params,
};

/// Returns `path` with the extension `.chrparams`.
pub(crate) fn params_path(skeleton: &str) -> String {
    let stem = skeleton.rsplit_once('.').map_or(skeleton, |(stem, _)| stem);
    format!("{stem}.chrparams")
}

impl ArchiveModels<'_> {
    pub(crate) fn pose_gear(
        &self,
        path: &str,
        model: &Model,
    ) -> Result<Option<DeployedGear>, ModelFailure> {
        let Some(entry) = self.entry_of(path) else {
            return Ok(None);
        };
        if !is_character_definition(&entry) {
            return Ok(None);
        }
        let observe = self.observes_model_sources();
        let (origin, bytes) = self.gear_file(&entry, MAX_CDF_BYTES, observe);
        let bytes = bytes?;
        let (skeleton, location) = if observe {
            cdf::read_skeleton_with_position(&bytes).map_err(unreadable)?
        } else {
            (cdf::read(&bytes).map_err(unreadable)?.skeleton, None)
        };
        let Some(skeleton) = skeleton else {
            return Ok(None);
        };
        let cdf_selector = origin.zip(location);
        let (params_origin, bytes) =
            self.gear_file(&params_path(&skeleton), MAX_CHRPARAMS_BYTES, observe);
        let read_parameters = if observe {
            gear_params::read_params_with_positions
        } else {
            read_params
        };
        let params = match bytes {
            Ok(bytes) => read_parameters(&bytes).map_err(unreadable)?,
            Err(ModelFailure::Missing) => return Ok(None),
            Err(other) => return Err(other),
        };
        let Some(clip) = params.deploy_clip() else {
            return Ok(None);
        };
        let candidates = ClipCandidates::of(self, &params, clip)?;
        if observe {
            let chosen = self
                .positioned_deploy_clip(clip, &candidates.paths)
                .map_err(|error| unreadable(error.to_string()))?
                .ok_or_else(|| missing_clip(clip))?;
            let selectors = DeploySelectors {
                cdf: cdf_selector,
                parameters: params_origin,
            };
            self.pose_observed(model, &chosen, selectors, &params, &candidates, clip)
                .map(Some)
        } else {
            let animation = self.read_clip(clip, &candidates.paths)?;
            let (model, _) = gear_pose::posed(model, &animation, None)
                .map_err(|failure| unreadable(failure.to_string()))?;
            Ok(Some(DeployedGear::from_model(model)))
        }
    }

    fn gear_file(
        &self,
        path: &str,
        limit: u64,
        observe: bool,
    ) -> (Option<EntryOrigin>, Result<Vec<u8>, ModelFailure>) {
        if observe {
            self.observed_file(path, limit)
        } else {
            (None, self.file(path, limit))
        }
    }

    fn read_clip(&self, clip: &str, paths: &[String]) -> Result<Animation, ModelFailure> {
        let choice = AnimationChoice::Named(vec![clip.to_owned()]);
        let set = self
            .animations_of(paths, &choice)
            .map_err(|error| unreadable(error.to_string()))?;
        set.animations
            .into_iter()
            .find(|animation| !animation.additive)
            .ok_or_else(|| missing_clip(clip))
    }

    fn pose_observed(
        &self,
        model: &Model,
        chosen: &ArchivePositionedAnimation,
        selectors: DeploySelectors,
        params: &ChrParams,
        candidates: &ClipCandidates,
        clip: &str,
    ) -> Result<DeployedGear, ModelFailure> {
        let positioned = chosen.animation();
        let (model, lods) = gear_pose::posed(model, positioned.animation(), Some(positioned))
            .map_err(|failure| unreadable(failure.to_string()))?;
        let route = candidates.used_route(self, clip, chosen.origin());
        let selectors = selectors.sources(params, route);
        Ok(DeployedGear {
            model,
            sources: Some(GearPoseSources {
                clip: chosen.origin().cloned(),
                clip_ordinal: positioned.clip_ordinal(),
                selectors,
                lods,
            }),
        })
    }
}

const fn unreadable(reason: String) -> ModelFailure {
    ModelFailure::Unreadable { reason }
}

fn missing_clip(clip: &str) -> ModelFailure {
    unreadable(format!("the clip {clip} is not in the animation database"))
}

struct ClipCandidates {
    paths: Vec<String>,
    database: bool,
    folder: bool,
}

impl ClipCandidates {
    fn of(
        models: &ArchiveModels<'_>,
        params: &ChrParams,
        clip: &str,
    ) -> Result<Self, ModelFailure> {
        let (folder, database) = params.last_sources();
        let database = database.and_then(|database| models.entry_of(database));
        let file = folder.and_then(|folder| {
            models.entry_of(&format!("{}/{clip}", folder.trim_end_matches('/')))
        });
        let candidates = Self {
            database: database.is_some(),
            folder: file.is_some(),
            paths: database.into_iter().chain(file).collect(),
        };
        if candidates.paths.is_empty() {
            return Err(unreadable(
                "the animation database and the clip are not in the Archive".to_owned(),
            ));
        }
        Ok(candidates)
    }

    fn used_route(
        &self,
        models: &ArchiveModels<'_>,
        clip: &str,
        selected: Option<&EntryOrigin>,
    ) -> Option<ParamRoute> {
        let selected = selected?.binding();
        let is_selected = |path| {
            models
                .animation_entry_binding(path)
                .is_some_and(|entry| entry == selected)
        };
        if is_selected(clip) {
            return None;
        }
        if self.database && self.paths.first().is_some_and(|path| is_selected(path)) {
            Some(ParamRoute::Database)
        } else if self.folder && self.paths.last().is_some_and(|path| is_selected(path)) {
            Some(ParamRoute::Folder)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
enum ParamRoute {
    Database,
    Folder,
}

struct DeploySelectors {
    cdf: Option<(EntryOrigin, SourceLocation)>,
    parameters: Option<EntryOrigin>,
}

impl DeploySelectors {
    fn sources(
        self,
        params: &ChrParams,
        route: Option<ParamRoute>,
    ) -> Vec<(EntryOrigin, SourceLocation)> {
        let mut sources: Vec<_> = self.cdf.into_iter().collect();
        if let Some(origin) = self.parameters {
            let mut add = |source: Option<&ParameterSource>, fields| {
                if let Some(source) = source {
                    sources.extend(
                        source
                            .locations(fields)
                            .map(|location| (origin.clone(), location)),
                    );
                }
            };
            add(params.deploy_source(), ["clipName", "clipPath"]);
            match route {
                Some(ParamRoute::Database) => add(
                    params.database_source.as_ref(),
                    ["databaseName", "databasePath"],
                ),
                Some(ParamRoute::Folder) => {
                    add(params.folder_source.as_ref(), ["folderName", "folderPath"]);
                }
                None => {}
            }
        }
        sources
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PARAM_SOURCES, params_path, read_params};

    const PARAMS: &str = r##"<Params><AnimationList>
        <Animation name="#filepath" path="Animations\Ships\Gear"/>
        <Animation name="$TracksDatabase" path="Animations/Ships/Gear.dba"/>
        <Animation name="gear_compress" path="front_compress.caf"/>
        <Animation name="gear_extend" path="front_extend.caf"/>
        <Animation name="gear_retract" path="front_retract.caf"/>
        </AnimationList></Params>"##;

    #[test]
    fn the_extending_clip_is_the_one_that_deploys() {
        let params = read_params(PARAMS.as_bytes()).expect("params");
        assert_eq!(params.folders, ["Animations/Ships/Gear"]);
        assert_eq!(params.databases, ["Animations/Ships/Gear.dba"]);
        assert_eq!(params.deploy_clip(), Some("front_extend.caf"));
    }

    #[test]
    fn deploy_is_taken_when_nothing_extends_and_compress_or_retract_never() {
        let text = r#"<Params><AnimationList>
            <Animation name="gear_retract" path="a_retract.caf"/>
            <Animation name="gear_compress" path="a_compress.caf"/>
            <Animation name="gear_deploy" path="a_deploy.caf"/>
            </AnimationList></Params>"#;
        assert_eq!(
            read_params(text.as_bytes()).expect("params").deploy_clip(),
            Some("a_deploy.caf")
        );
        let none = r#"<Params><AnimationList>
            <Animation name="gear_retract" path="a_retract.caf"/>
            </AnimationList></Params>"#;
        assert_eq!(
            read_params(none.as_bytes()).expect("params").deploy_clip(),
            None
        );
    }

    #[test]
    fn a_document_that_is_no_parameter_file_is_refused() {
        assert!(read_params(b"<Other/>").is_err());
        assert!(read_params(b"not xml").is_err());
    }

    const SEVERAL: &str = r##"<Params><AnimationList>
        <Animation name="#filepath" path="Animations\First"/>
        <Animation name="$TracksDatabase" path="Animations/First.dba"/>
        <Animation name="#filepath" path="Animations/Second"/>
        <Animation name="$TracksDatabase" path="Animations/Second.dba"/>
        </AnimationList></Params>"##;

    #[test]
    fn every_folder_and_database_is_kept_in_file_order() {
        let params = read_params(SEVERAL.as_bytes()).expect("params");

        assert_eq!(params.folders, ["Animations/First", "Animations/Second"]);
        assert_eq!(
            params.databases,
            ["Animations/First.dba", "Animations/Second.dba"]
        );
        assert!(!params.capped);
    }

    #[test]
    fn a_landing_gear_takes_the_last_folder_and_database() {
        let params = read_params(SEVERAL.as_bytes()).expect("params");

        assert_eq!(
            params.last_sources(),
            (Some("Animations/Second"), Some("Animations/Second.dba"))
        );
    }

    #[test]
    fn folders_beyond_the_cap_are_dropped_but_the_last_stays_last() {
        let list: Vec<String> = (0..=MAX_PARAM_SOURCES)
            .map(|at| format!(r##"<Animation name="#filepath" path="Animations/f{at}"/>"##))
            .collect();
        let text = format!(
            "<Params><AnimationList>{}</AnimationList></Params>",
            list.concat()
        );

        let params = read_params(text.as_bytes()).expect("params");

        assert_eq!(params.folders.len(), MAX_PARAM_SOURCES);
        assert!(params.capped);
        assert_eq!(
            params.last_sources().0,
            Some(format!("Animations/f{MAX_PARAM_SOURCES}").as_str())
        );
    }

    #[test]
    fn the_parameter_file_sits_beside_the_skeleton() {
        assert_eq!(
            params_path("Objects/a/Gear_CHR.chr"),
            "Objects/a/Gear_CHR.chrparams"
        );
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

macro_rules! ui_texts {
    ($($(#[$doc:meta])* $name:ident = $id:literal;)*) => {
        /// A text of the UI tab, looked up in the Fluent file of the current language.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum UiText {
            $($(#[$doc])* $name,)*
        }

        impl UiText {
            /// Every text of the UI tab.
            pub(crate) const ALL: [Self; { [$(Self::$name),*].len() }] = [$(Self::$name),*];

            /// Returns the Fluent message id.
            #[must_use]
            pub const fn id(self) -> &'static str {
                match self {
                    $(Self::$name => $id,)*
                }
            }
        }
    };
}

ui_texts! {
    /// The tab.
    Tab = "ui-tab";
    /// The label of the choice between UI files and ships.
    ModeLabel = "ui-mode-label";
    /// The choice of the GFx and SWF files of the Archive.
    ModeFiles = "ui-mode-files";
    /// The choice of the screens of a ship.
    ModeShips = "ui-mode-ships";
    /// The label of the search field.
    SearchLabel = "ui-search-label";
    /// The hint inside the empty search field of the files.
    FilesHint = "ui-files-hint";
    /// The hint inside the empty search field of the ships.
    ShipsHint = "ui-ships-hint";
    /// The title of the empty state before an Archive is open.
    EmptyTitle = "ui-empty-title";
    /// The hint of the empty state.
    EmptyHint = "ui-empty-hint";
    /// Shown while the files are looked up.
    FilesLoading = "ui-files-loading";
    /// Shown when the Archive holds no GFx or SWF file.
    FilesNone = "ui-files-none";
    /// Looking the files up failed; takes `reason`.
    FilesFailed = "ui-files-failed";
    /// Looking the files up stopped without a result.
    FilesStopped = "ui-files-stopped";
    /// The Archive changed on disk, so the files were not listed.
    FilesArchiveChanged = "ui-files-archive-changed";
    /// The number of files; takes `count` and `number`.
    FilesCount = "ui-files-count";
    /// The number of files when the search leaves some; takes `count`, `number` and `total`.
    FilesCountShown = "ui-files-count-shown";
    /// Shown when the DataForge holds no ship or vehicle.
    ShipsNone = "ui-ships-none";
    /// The number of ships and vehicles; takes `count` and `number`.
    ShipsCount = "ui-ships-count";
    /// The number of ships and vehicles when the search leaves some; takes `count`, `number`
    /// and `total`.
    ShipsCountShown = "ui-ships-count-shown";
    /// Shown when nothing matches the search.
    NoMatch = "ui-no-match";
    /// The heading of the list of screens.
    ScreensHeading = "ui-screens-heading";
    /// Shown while no ship is picked.
    ScreensPick = "ui-screens-pick";
    /// Shown while the screens of the ship are listed.
    ScreensLoading = "ui-screens-loading";
    /// Listing the screens failed; takes `reason`.
    ScreensFailed = "ui-screens-failed";
    /// Listing the screens stopped without a result.
    ScreensStopped = "ui-screens-stopped";
    /// The Archive changed on disk, so the screens were not listed.
    ScreensArchiveChanged = "ui-screens-archive-changed";
    /// Shown when the ship has no screens.
    ScreensNone = "ui-screens-none";
    /// The name of a screen the ship itself shows.
    ScreenOwn = "ui-screen-own";
    /// The size of a screen; takes `width` and `height`.
    ScreenSize = "ui-screen-size";
    /// The label of the size choice.
    SizeLabel = "ui-size-label";
    /// The size of the stage or the canvas.
    SizeStage = "ui-size-stage";
    /// 1024 pixels wide.
    SizeWide1024 = "ui-size-wide-1024";
    /// 2048 pixels wide.
    SizeWide2048 = "ui-size-wide-2048";
    /// 1920 by 1080 pixels.
    SizeFullHd = "ui-size-full-hd";
    /// The label of the background choice.
    BackgroundLabel = "ui-background-label";
    /// No background.
    BackgroundTransparent = "ui-background-transparent";
    /// The background colour of the movie.
    BackgroundMovie = "ui-background-movie";
    /// A black background.
    BackgroundBlack = "ui-background-black";
    /// A white background.
    BackgroundWhite = "ui-background-white";
    /// The label of the choice of the game state a screen is drawn in.
    PowerLabel = "ui-power-label";
    /// Every game variable at zero.
    PowerAtRest = "ui-power-at-rest";
    /// The ship is powered on.
    PowerOn = "ui-power-on";
    /// The label of the field for more game variables.
    VariablesLabel = "ui-variables-label";
    /// The hint inside the empty field for more game variables.
    VariablesHint = "ui-variables-hint";
    /// Settings of the field that are no `NAME=VALUE`; takes `settings`.
    VariablesInvalid = "ui-variables-invalid";
    /// The label of the format choice.
    FormatLabel = "ui-format-label";
    /// The PNG format.
    FormatPng = "ui-format-png";
    /// The SVG format.
    FormatSvg = "ui-format-svg";
    /// The action that exports the picture to a file.
    Export = "ui-export";
    /// Shown while the export runs; takes `file`.
    ExportRunning = "ui-export-running";
    /// The export wrote the file; takes `file`, `width` and `height`.
    ExportDone = "ui-export-done";
    /// The export wrote the file but left elements out; takes `file`, `width`, `height`, `count`
    /// and `number`.
    ExportDoneIncomplete = "ui-export-done-incomplete";
    /// The committed origin companion path; takes `file`.
    ExportOriginReceipt = "ui-export-origin-receipt";
    /// The export failed; takes `reason`.
    ExportFailed = "ui-export-failed";
    /// The export stopped without a result.
    ExportStopped = "ui-export-stopped";
    /// The export was refused because the file is the open Archive.
    ExportIsArchive = "ui-export-is-archive";
    /// The Archive changed on disk, so nothing was exported.
    ExportArchiveChanged = "ui-export-archive-changed";
    /// The file was written, but the Archive changed on disk meanwhile; takes `file`.
    ExportWrittenArchiveChanged = "ui-export-written-archive-changed";
    /// The action that dismisses the outcome of an export.
    ExportDismiss = "ui-export-dismiss";
    /// The name of the PNG file type in the save dialog.
    FilterPng = "ui-filter-png";
    /// The name of the SVG file type in the save dialog.
    FilterSvg = "ui-filter-svg";
    /// Shown while a picture is drawn.
    Rendering = "ui-rendering";
    /// Drawing failed; takes `reason`.
    RenderFailed = "ui-render-failed";
    /// Drawing stopped without a result.
    RenderStopped = "ui-render-stopped";
    /// The Archive changed on disk, so nothing was drawn.
    RenderArchiveChanged = "ui-render-archive-changed";
    /// Shown while no file is picked.
    PickFile = "ui-pick-file";
    /// Shown while no screen is picked.
    PickScreen = "ui-pick-screen";
    /// The picture is too large to show; takes `width` and `height`.
    TooLarge = "ui-too-large";
    /// The accessible name of the picture; takes `width` and `height`.
    Shown = "ui-shown";
    /// The tab of the contents.
    DetailContents = "ui-detail-contents";
    /// The tab of the elements left out.
    DetailReport = "ui-detail-report";
    /// Shown when there is nothing to list.
    ContentsNone = "ui-contents-none";
    /// Shown when nothing was left out.
    ReportNone = "ui-report-none";
    /// The note below a report cut short; takes `number`, the lines not shown.
    ReportMore = "ui-report-more";
    /// A bitmap held as lossless pixels; takes `size`.
    NoteLossless = "ui-note-lossless";
    /// A bitmap held as an embedded image.
    NoteEmbedded = "ui-note-embedded";
    /// A bitmap loaded from a file; takes `size`.
    NoteExternal = "ui-note-external";
    /// A bitmap that is a part of another one.
    NotePart = "ui-note-part";
    /// A font; takes `name`, `count` and `number`, the glyphs.
    NoteFont = "ui-note-font";
    /// A sprite; takes `count` and `number`, the frames.
    NoteSprite = "ui-note-sprite";
    /// The format of a movie.
    FactFormat = "ui-fact-format";
    /// The stage of a movie.
    FactStage = "ui-fact-stage";
    /// The frames of a movie.
    FactFrames = "ui-fact-frames";
    /// The tags of a movie.
    FactTags = "ui-fact-tags";
    /// The shapes of a movie.
    FactShapes = "ui-fact-shapes";
    /// The bitmaps of a movie.
    FactBitmaps = "ui-fact-bitmaps";
    /// The fonts of a movie.
    FactFonts = "ui-fact-fonts";
    /// The sprites of a movie.
    FactSprites = "ui-fact-sprites";
    /// The Entity that holds a screen.
    FactEntity = "ui-fact-entity";
    /// The port a screen sits in.
    FactPort = "ui-fact-port";
    /// The size of a screen.
    FactSize = "ui-fact-size";
    /// The canvases a screen uses.
    FactCanvases = "ui-fact-canvases";
    /// The textures a screen uses.
    FactTextures = "ui-fact-textures";
    /// The vector graphics a screen uses.
    FactVectors = "ui-fact-vectors";
    /// The movies a screen uses.
    FactMovies = "ui-fact-movies";
    /// The localisation keys a screen uses.
    FactKeys = "ui-fact-keys";
    /// The widgets of a drawn screen.
    FactWidgets = "ui-fact-widgets";
    /// The texts of a screen that were drawn.
    FactTextsDrawn = "ui-fact-texts-drawn";
    /// The texts of a screen that were left out.
    FactTextsLeftOut = "ui-fact-texts-left-out";
    /// A shape.
    KindShape = "ui-kind-shape";
    /// A morph shape.
    KindMorphShape = "ui-kind-morph-shape";
    /// A bitmap.
    KindBitmap = "ui-kind-bitmap";
    /// A font.
    KindFont = "ui-kind-font";
    /// A sprite.
    KindSprite = "ui-kind-sprite";
    /// Assets imported from another movie.
    KindImport = "ui-kind-import";
    /// A canvas.
    KindCanvas = "ui-kind-canvas";
    /// A texture.
    KindTexture = "ui-kind-texture";
    /// A vector graphic.
    KindVector = "ui-kind-vector";
    /// A movie.
    KindMovie = "ui-kind-movie";
    /// A localisation key.
    KindKey = "ui-kind-key";
    /// Widgets of one type that the renderer draws.
    KindElement = "ui-kind-element";
    /// Widgets of one type that the renderer does not draw.
    KindElementUndrawn = "ui-kind-element-undrawn";
    /// An element the renderer skipped.
    ReportSkipped = "ui-report-skipped";
    /// An element drawn without an effect it asks for.
    ReportEffect = "ui-report-effect";
    /// Widgets or features of one type that were left out.
    ReportLeftOut = "ui-report-left-out";
    /// The heading for individual content omissions.
    ReportOmissionDetails = "ui-report-omission-details";
    /// The original widget, field and source of an omission.
    ReportOmissionRow = "ui-report-omission-row";
    /// An omission without an authored source identifier.
    ReportOmissionNoSource = "ui-report-omission-no-source";
    /// The widget field names no source.
    ReportOmissionSourceMissing = "ui-report-omission-source-missing";
    /// The image requires a game render target.
    ReportOmissionRuntimeTarget = "ui-report-omission-runtime-target";
    /// The list requires game data.
    ReportOmissionRuntimeArray = "ui-report-omission-runtime-array";
    /// A variable has no supplied value.
    ReportOmissionVariableMissing = "ui-report-omission-variable-missing";
    /// A binding targets an unsupported field.
    ReportOmissionUnsupportedField = "ui-report-omission-unsupported-field";
    /// The renderer could not evaluate the input.
    ReportOmissionUnresolvedInput = "ui-report-omission-unresolved-input";
    /// An omission with an unrecognised reason.
    ReportOmissionOther = "ui-report-omission-other";
    /// A nested canvas that could not be read.
    ReportMissingCanvas = "ui-report-missing-canvas";
    /// A texture that the Archive does not hold or cannot read.
    ReportMissingTexture = "ui-report-missing-texture";
    /// A game variable that was set and that no canvas of the screen reads.
    ReportUnknownVariable = "ui-report-unknown-variable";
    /// A limit stopped the composition.
    ReportTruncated = "ui-report-truncated";
    /// A font that could not be imported.
    ReportImport = "ui-report-import";
    /// A tag of the movie that could not be read.
    ReportTag = "ui-report-tag";
    /// A bitmap that could not be decoded.
    ReportBitmap = "ui-report-bitmap";
    /// Something the texts are drawn without.
    ReportWarning = "ui-report-warning";
}

impl From<UiText> for crate::Text {
    fn from(text: UiText) -> Self {
        Self::Ui(text)
    }
}

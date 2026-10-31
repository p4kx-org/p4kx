app-title = P4KX
menu-archive = Archiv
action-open = Archiv öffnen …
action-open-dataforge = DataForge-Datei öffnen …
action-add-folder = Ordner hinzufügen …
action-add-archive = Archiv hinzufügen …
menu-recent = Zuletzt geöffnet
recent-none = Noch kein Archiv geöffnet
channels-heading = Channels
channels-none = Keine Channels gefunden. Füge über das Menü Archiv einen Ordner oder ein Archiv hinzu.
sidebar-show = Seitenleiste einblenden
sidebar-hide = Seitenleiste ausblenden
menu-language = Sprache
language-de-DE = Deutsch
language-en-GB = English (UK)
menu-help = Hilfe
font-licence = Schriftlizenz
update-check = Beim Start nach neuer Version suchen
update-available = Version { $version } ist verfügbar
dialog-archive-filter = Star-Citizen-Archiv
dialog-dataforge-filter = DataForge-Datenbank
column-name = Name
column-compression = Kompression
column-encryption = Verschlüsselung
column-size = Größe
select-all = Alle angezeigten Einträge auswählen
select-entry = { $name } auswählen
compression-store = Unkomprimiert
compression-deflate = Deflate
compression-zstd = Zstandard
compression-other = Methode { $method }
encrypted-yes = Ja
encrypted-no = Nein
expand-folder = { $name } aufklappen
collapse-folder = { $name } zuklappen
search-label = Suche
search-hint = Nach Pfad filtern: Glob mit * oder ?, sonst ein beliebiger Teil des Pfads
drop-hint = Data.p4k hier ablegen, um sie zu öffnen
empty-title = Kein Archiv geöffnet
drop-release = Loslassen zum Öffnen
quick-filters = Schnellfilter
close-dialog = Dialog schließen
shortcut-reload = F5
shortcut-extract-selected = Strg+E
status-empty = Öffne ein Archiv oder ziehe eine Data.p4k in das Fenster.
status-opening = Öffne { $path } …
status-failed = { $path } lässt sich nicht öffnen: { $reason }
status-opening-stopped = Das Öffnen von { $path } brach ohne Ergebnis ab.
status-entries =
    { $count ->
        [one] { $number } Eintrag
       *[other] { $number } Einträge
    }
status-shown = { $number } angezeigt
status-selected = { $number } ausgewählt
status-selected-hidden = { $number } ausgewählt, davon { $hidden } nicht angezeigt
status-searching = Suche …
status-build = Build { $build }
status-build-unknown = Build unbekannt
preview-tab-text = Text
preview-tab-hex = Hex
preview-copy = Kopieren
preview-none = Einen Eintrag auswählen, um ihn in der Vorschau zu sehen.
preview-loading = { $path } wird gelesen …
preview-too-large = { $path } ist { $size } groß. Einträge über { $limit } werden nicht in der Vorschau gezeigt; zum Ansehen extrahieren.
preview-truncated = Gekürzt: angezeigt werden die ersten { $shown } von { $total } Bytes.
preview-failed = { $path } kann nicht gelesen werden: { $reason }
preview-stopped = Das Lesen von { $path } brach ohne Ergebnis ab.
preview-archive-changed = Das Archiv hat sich auf der Festplatte geändert. Lade es neu, um Einträge anzusehen.
archive-changed =
    { $stopped ->
        [yes] { $path } hat sich auf der Festplatte geändert, vermutlich durch ein Update im RSI Launcher. Eine laufende Extraktion wurde gestoppt; lade das Archiv neu, um wieder Einträge zu extrahieren oder anzusehen.
       *[no] { $path } hat sich auf der Festplatte geändert, vermutlich durch ein Update im RSI Launcher. Lade das Archiv neu, um wieder Einträge zu extrahieren oder anzusehen.
    }
archive-issues =
    { $kind ->
        [one] Das Verzeichnis des Archivs hat einen Fehler in einem Eintrag. Der Eintrag bleibt aufgelistet, seine Daten sind aber womöglich nicht lesbar; eine Extraktion meldet ihn, wenn er scheitert, und `p4kx info` nennt ihn.
       *[other] Das Verzeichnis des Archivs hat { $count } Fehler in seinen Einträgen. Die Einträge bleiben aufgelistet, ihre Daten sind aber womöglich nicht lesbar; eine Extraktion meldet die gescheiterten, und `p4kx info` nennt sie.
    }
game-running = Star Citizen läuft. Nutze dieses Werkzeug bei beendetem Spiel: Lesen funktioniert weiter, aber der RSI Launcher kann das Archiv beim Aktualisieren ändern, und das Werkzeug schreibt nie in die Installation.
archive-test-build =
    { $kind ->
        [one] Dieses Archiv gehört zum Channel { $channel }, der Testbuilds von Star Citizen enthält. Material aus Testbuilds ist nach den Nutzungsbedingungen von Star Citizen vertraulich: Gib nichts daraus weiter und veröffentliche nichts davon.
       *[other] Diese Archive gehören zu den Channels { $channel }, die Testbuilds von Star Citizen enthalten. Material aus Testbuilds ist nach den Nutzungsbedingungen von Star Citizen vertraulich: Gib nichts daraus weiter und veröffentliche nichts davon.
    }
archive-test-build-nda = Der Inhalt von EPTU kann zusätzlich einer Geheimhaltungsvereinbarung unterliegen und darf nicht weitergegeben werden.
action-reload = Neu laden
extract-selected = Ausgewählte extrahieren …
extract-shown = Angezeigte extrahieren …
extract-all = Alle extrahieren …
extraction-setup-title = Einträge extrahieren
extraction-scope-heading = Einträge
extraction-scope-selected = Ausgewählte ({ $number })
extraction-scope-shown = Angezeigte ({ $number })
extraction-scope-all = Ganzes Archiv ({ $number })
extraction-exclude-heading = Auslassen
extraction-exclude-hint = Ein Filter je Zeile: Glob mit * oder ?, sonst ein beliebiger Teil des Pfads
filter-too-long = Ein Filter ist länger als { $max } Bytes.
filter-too-many-wildcards = Ein Filter hat mehr als { $max } Platzhalter (* und ?).
extraction-exclude-too-many = Es sind mehr als { $max } Filter.
extraction-exclude-empty = Eine Zeile besteht nur aus Leerzeichen; entferne sie.
extraction-exclude-nothing-left = Diese Filter lassen keinen Eintrag zum Extrahieren übrig.
extraction-target-heading = Ziel
extraction-target-new = Neuer Ordner in
extraction-target-existing = Vorhandener Ordner
extraction-target-new-name = Legt { $name } an
extraction-target-in-installation = Dieser Ordner liegt in der Star-Citizen-Installation { $installation }. Das Werkzeug schreibt dort nie; wähle einen Ordner an anderer Stelle.
extraction-folder-none = Kein Ordner gewählt
extraction-choose-folder = Ordner wählen …
extraction-policy-heading = Bereits vorhandene Dateien
extraction-policy-skip = Überspringen
extraction-policy-overwrite = Überschreiben
extraction-policy-if-changed = Nur bei Änderung überschreiben
extraction-conversion = CryXml und DataForge in XML, Texturen und Sounds umwandeln
extraction-texture-merge = Geteilte Texturen zu einer DDS zusammenführen
extraction-texture-png = Texturen als PNG exportieren (HDR als OpenEXR)
extraction-texture-mip-largest = Größte Mip-Stufe
extraction-texture-mip-level = Mip-Stufe
extraction-texture-raw = Rohe Texturteile behalten
extraction-audio-label = Sounds als
extraction-audio-ogg = Ogg
extraction-audio-wav = WAV
extraction-audio-raw = WEM, unverändert
extraction-audio-banks = Ohne Ogg-Form des Codecs wird statt Ogg WAV geschrieben. Soundbanks werden zusätzlich in einen Ordner je Bank entpackt, jede WEM nach ihrer ID benannt.
extraction-start = Extrahieren
extraction-close = Schließen
extraction-confirm-all =
    { $count ->
        [one] Das ganze Archiv extrahieren, { $number } Eintrag mit { $size }?
       *[other] Das ganze Archiv extrahieren, { $number } Einträge mit { $size }?
    }
extraction-confirm-all-action = Ganzes Archiv extrahieren
extraction-back = Zurück
extraction-running-title = Extrahiere
extraction-progress = { $done } von { $total } Einträgen, { $percent } %
extraction-cancelling = Breche ab …
extraction-cancel = Abbrechen
extraction-summary-title = Extraktion abgeschlossen
extraction-summary-cancelled-title = Extraktion abgebrochen
extraction-summary-changed-title = Extraktion gestoppt: Das Archiv hat sich geändert
extraction-summary-changed = Das Archiv hat sich während der Extraktion auf der Festplatte geändert. Bereits geschriebene Dateien können veraltet sein; lade das Archiv neu und extrahiere erneut.
extraction-summary-counts = Extrahiert { $extracted }, übersprungen { $skipped }, fehlgeschlagen { $failed }, abgebrochen { $cancelled }
extraction-summary-counts-converted = Extrahiert { $extracted } ({ $converted } umgewandelt), übersprungen { $skipped }, fehlgeschlagen { $failed }, abgebrochen { $cancelled }
extraction-summary-excluded-parts =
    { $count ->
        [one] { $number } ausgelassener Teil-Eintrag einer Textur, die blieb, wurde zum Umwandeln der Textur trotzdem gelesen.
       *[other] { $number } ausgelassene Teil-Einträge von Texturen, die blieben, wurden zum Umwandeln der Texturen trotzdem gelesen.
    }
extraction-summary-records = Records geschrieben { $written }, übersprungen { $skipped }, fehlgeschlagen { $failed }, abgebrochen { $cancelled }; { $errors } Fehlerwerte in { $erroneous } Records
extraction-summary-target = Ziel: { $path }
extraction-summary-failed = Fehlgeschlagen ({ $number })
extraction-summary-skipped = Übersprungen ({ $number })
extraction-summary-cancelled = Abgebrochen ({ $number })
extraction-summary-unconverted = Nicht umgewandelt ({ $number })
extraction-summary-failure = { $path }: { $reason }
extraction-summary-records-failed = Nicht geschriebene Records ({ $number })
extraction-summary-record-failure = { $path } ({ $guid }): { $reason }
rename-heading = Umbenannte Records ({ $number })
rename-line = { $path } ({ $guid }): { $reason }
rename-name = der Record-Name ist kein gültiger Dateiname ({ $rule }); die GUID benennt die Datei
rename-path = der Record-Pfad ist kein gültiger Dateiname ({ $rule }); der Dateiname wird maskiert
rename-rule-empty = der Name ist leer
rename-rule-dot-part = der Name ist ein Punktteil
rename-rule-forbidden-character = das Zeichen { $char } ist in Dateinamen nicht erlaubt
rename-rule-trailing-dot-or-space = der Name endet auf einen Punkt oder ein Leerzeichen
rename-rule-device-name = der Name ist ein Windows-Gerätename
rename-rule-too-many-bytes = der Name hat { $bytes } Bytes in UTF-8, mehr als { $max }
extraction-failed = Das Ziel in { $path } lässt sich nicht anlegen: { $reason }
extraction-stopped = Die Extraktion nach { $path } brach ohne Ergebnis ab.
extraction-failed-title = Extraktion fehlgeschlagen
comparison-tab-archive = Durchsuchen
comparison-tab-comparison = Vergleich
comparison-old-archive = Altes Archiv
comparison-new-archive = Neues Archiv
comparison-pick-archive = Archiv wählen
comparison-choose-file = Datei wählen …
comparison-levels = Ebenen
comparison-level-entries = Einträge
comparison-level-records = Records
comparison-entries-skipped = Die Einträge wurden nicht verglichen; schalte Einträge ein, um sie aufzulisten.
comparison-start = Vergleichen
comparison-cancel = Abbrechen
comparison-opening = Beide Archive werden geöffnet …
comparison-progress = { $done } von { $total } Einträgen, { $percent } %
comparison-preparing = Die Liste wird vorbereitet …
comparison-cancelled = Der Vergleich wurde abgebrochen.
comparison-failed = Der Vergleich ist fehlgeschlagen: { $reason }
comparison-stopped = Der Vergleich endete ohne Ergebnis.
comparison-empty-title = Noch kein Vergleich
comparison-empty-hint = Wähle ein altes und ein neues Archiv und vergleiche sie.
comparison-fact-old = Alt
comparison-fact-new = Neu
comparison-side = { $channel } · { $build }
comparison-channel-unknown = Kein Channel
comparison-added = Hinzugefügt
comparison-removed = Entfernt
comparison-changed = Geändert
comparison-directories = Nach Verzeichnis ({ $number })
comparison-root = (Wurzel)
comparison-column-directory = Verzeichnis
comparison-column-change = Änderung
comparison-column-path = Pfad
comparison-column-old-size = Alte Größe
comparison-column-new-size = Neue Größe
comparison-no-size = –
comparison-save-report = Bericht speichern …
comparison-report-filter = JSON-Bericht
comparison-report-saving = Der Bericht wird in { $path } gespeichert …
comparison-report-saved = Bericht gespeichert in { $path }
comparison-report-failed = Der Bericht kann nicht in { $path } gespeichert werden: { $reason }
comparison-no-differences = Kein Eintrag unterscheidet sich zwischen den Archiven.
comparison-no-matches = Keine Änderung passt zum Filter.
comparison-shown = { $number } angezeigt
comparison-row = { $change }: { $path }

schema-tab = Schema
schema-label-structs = Structs
schema-label-properties = Eigenschaften
schema-label-enums = Enums
schema-label-options = Enum-Optionen
schema-counts = { $label }: { $added } hinzugefügt, { $removed } entfernt, { $changed } geändert
schema-column-kind = Art
schema-column-name = Name
schema-column-old = Alt
schema-column-new = Neu
schema-column-old-records = Records alt
schema-column-new-records = Records neu
schema-kind-struct = Struct
schema-kind-parent = Basis-Struct
schema-kind-property = Eigenschaft
schema-kind-enum = Enum
schema-kind-option = Option
schema-no-differences = Das Schema unterscheidet sich zwischen den DataForges nicht.
schema-failed = Die Schemas konnten nicht verglichen werden: { $reason }
schema-row ={ $change }: { $kind } { $name }

diff-previous = Vorheriger Unterschied
diff-next = Nächster Unterschied
diff-position = Unterschied { $current } von { $total }
diff-head-old = Alt
diff-head-new = Neu
diff-hunk = Zeilen { $first }–{ $last }
diff-hunk-empty = Keine Zeilen
diff-loading = Zeilenvergleich von { $path } wird gelesen …
diff-binary = { $path } ist binär, daher gibt es keinen Zeilenvergleich.
diff-too-large = { $path } ist zu groß für einen Zeilenvergleich: Eine Seite hat mehr als { $size } oder { $lines } Zeilen.
diff-failed = { $path } lässt sich nicht vergleichen: { $reason }
diff-stopped = Der Zeilenvergleich von { $path } endete ohne Ergebnis.
diff-equal = Die Zeilen beider Seiten sind gleich; nur die gespeicherten Bytes unterscheiden sich.
diff-line-context = Zeile { $line }: { $text }
diff-line-removed = Entfernte Zeile { $line }: { $text }
diff-line-added = Hinzugefügte Zeile { $line }: { $text }
diff-marker-removed = −
diff-marker-added = +
query-tab = Abfrage
query-label = Abfrage
query-hint = from EntityClassDefinition at "*/spaceships/*" select Category
query-run = Abfrage ausführen
query-cancel = Abbrechen
query-loading = DataForge wird gelesen …
query-indexing = Referenzen werden indiziert …
query-selecting = Records werden ausgewählt …
query-progress = { $done } von { $total } Records, { $percent } %
query-cancelled = Die Abfrage wurde abgebrochen.
query-failed = Die Abfrage ist fehlgeschlagen: { $reason }
query-stopped = Die Abfrage wurde ohne Ergebnis beendet.
query-archive-changed = Das Archiv wurde auf der Festplatte geändert. Laden Sie es neu, um Abfragen auszuführen.
query-invalid = Zeile { $line }, Spalte { $column }: { $reason }
query-empty-title = Kein Archiv geöffnet
query-empty-hint = Öffnen Sie ein Archiv, um Abfragen über sein DataForge auszuführen.
query-prompt = Schreiben Sie eine Abfrage und führen Sie sie mit Strg+Enter aus, z. B. from AmmoParams select speed where speed >= 1500
query-no-rows = Kein Record passt zur Abfrage.
query-rows =
    { $count ->
        [one] { $number } Record
       *[other] { $number } Records
    }
query-rows-failed =
    { $count ->
        [one] { $number } Record, { $failed } fehlgeschlagen
       *[other] { $number } Records, { $failed } fehlgeschlagen
    }
query-unresolved =
    { $count ->
        [one] , { $number } nicht aufgelöste Referenz
       *[other] , { $number } nicht aufgelöste Referenzen
    }
query-export-csv = CSV exportieren …
query-export-json = JSON exportieren …
query-filter-csv = CSV-Datei
query-filter-json = JSON-Datei
query-exporting = Das Ergebnis wird nach { $path } exportiert …
query-exported =
    { $count ->
        [one] { $number } Record nach { $path } exportiert
       *[other] { $number } Records nach { $path } exportiert
    }
query-export-failed = Das Ergebnis kann nicht nach { $path } exportiert werden: { $reason }
query-export-cancel = Export abbrechen
query-export-cancelled = Der Export nach { $path } wurde abgebrochen.
query-page = Seite { $page } von { $pages }
query-page-first = Erste Seite
query-page-previous = Vorherige Seite
query-page-next = Nächste Seite
query-page-last = Letzte Seite
query-completion-more =
    { $count ->
        [one] { $number } weiterer Name, weiter tippen
       *[other] { $number } weitere Namen, weiter tippen
    }
query-save = Abfrage speichern …
query-saved-title = Gespeicherte Abfragen
query-saved-none = Noch keine gespeicherten Abfragen. Speichern Sie die Abfrage im Editor, um sie mit einem Klick erneut auszuführen.
query-saved-run = Gespeicherte Abfrage { $name } ausführen
query-saved-rename = Umbenennen
query-saved-delete = Löschen
query-save-title = Abfrage speichern
query-rename-title = Gespeicherte Abfrage umbenennen
query-delete-title = Gespeicherte Abfrage löschen
query-name-label = Name
query-name-hint = z. B. Schwere Schiffe
query-save-action = Speichern
query-rename-action = Umbenennen
query-delete-action = Löschen
query-delete-question = Die gespeicherte Abfrage „{ $name }“ wird gelöscht. Die Abfrage im Editor bleibt.
query-name-replaces = Ersetzt die gespeicherte Abfrage dieses Namens.
query-name-empty = Geben Sie einen Namen ein.
query-name-too-long = Der Name ist länger als { $max } Zeichen.
query-name-taken = Eine andere gespeicherte Abfrage hat diesen Namen.
query-name-control = Der Name darf keine Zeilenumbrüche oder anderen Steuerzeichen enthalten.
query-name-full = Die gespeicherten Abfragen sind voll: höchstens { $max } Abfragen und { $size } insgesamt. Löschen Sie zuerst eine.
query-name-bad-text = Die Abfrage im Editor ist leer oder zu lang zum Speichern.

dataforge-node = DataForge
dataforge-node-loading = Wird geladen …
preview-tab-dataforge = DataForge
dataforge-loading = DataForge wird aus { $path } geladen …
dataforge-unsupported = DataForge-Version { $version } wird von dieser Version von P4KX nicht unterstützt.
dataforge-failed = DataForge lässt sich nicht aus { $path } laden: { $reason }
dataforge-stopped = Das Laden von DataForge aus { $path } brach ohne Ergebnis ab.
dataforge-archive-changed = Das Archiv hat sich auf der Festplatte geändert. Lade es neu, um DataForge zu laden.
dataforge-version = Version
dataforge-records = Records
record-search-label = Records suchen
record-search-hint = Nach Record-Pfad oder Typ filtern: Glob mit * oder ?, sonst ein beliebiger Teil
record-search-prompt = Gib einen Teil eines Record-Pfads oder Typnamens ein, um Records zu finden.
record-search-no-matches = Kein Record passt zur Suche.
record-search-matches =
    { $count ->
        [one] { $number } Record
       *[other] { $number } Records
    }
dataforge-structs = Structs
dataforge-enums = Enums
dataforge-first-records = Erste Records
column-type = Typ
column-record-path = Record-Pfad
record-change-progress = { $done } von { $total } Records, { $percent } %
record-change-moved = Verschoben
record-change-unreadable = Unlesbar
record-change-moved-path = { $old } zu { $new }
record-change-no-differences = Kein Record unterscheidet sich zwischen den DataForges.
record-change-head-field = Feld
record-change-field-count =
    { $count ->
        [one] { $number } Änderung
       *[other] { $number } Änderungen
    }
record-change-loading = Die Felder von { $path } werden verglichen …
record-change-failed = Die Felder von { $path } lassen sich nicht vergleichen: { $reason }
record-change-stopped = Der Feldvergleich von { $path } endete ohne Ergebnis.
record-change-only-new = Den Record gibt es nur im neuen Archiv.
record-change-only-old = Den Record gibt es nur im alten Archiv.
record-change-only-path = Nur der Record-Pfad hat sich geändert; die Felder sind gleich.
record-change-no-field-changes = Kein Feld unterscheidet sich.
record-change-unreadable-old = Der alte Record lässt sich nicht lesen: { $reason }
record-change-unreadable-new = Der neue Record lässt sich nicht lesen: { $reason }
record-change-record-name = Record-Name
record-change-record-tag = Record-Tag
record-change-record-type = Record-Typ
record-change-value-missing = –
record-change-value-null = null
record-change-value-struct = { $type } (Struct)
record-change-value-array =
    { $count ->
        [one] { $number } Element
       *[other] { $number } Elemente
    }
record-change-value-reference-at = { $path } · { $at }
record-change-value-error = Fehler: { $reason }
record-change-field-row = { $field }: { $old } zu { $new }
change-extraction-action-old = Alt extrahieren ({ $number }) …
change-extraction-action-new = Neu extrahieren ({ $number }) …
change-extraction-scope-old = Geänderte Einträge des alten Archivs, in ihrer alten Fassung ({ $number })
change-extraction-scope-new = Hinzugefügte und geänderte Einträge des neuen Archivs ({ $number })
change-extraction-scope-filtered = Nur die Änderungen, die zum Filter { $filter } passen
change-extraction-summary-changed = Das Archiv hat sich während der Extraktion auf der Festplatte geändert. Bereits geschriebene Dateien können veraltet sein; vergleiche die Archive erneut und extrahiere die Änderungen noch einmal.

preview-tab-image = Bild
image-mip = Mip
image-format = Format
image-size = Größe
image-mips = Mips
image-faces = Cubemap-Seiten
image-layers = Ebenen
image-dimensions = { $width } × { $height }
image-none = –
image-shown = Mip { $level }: { $width } × { $height } Pixel
image-decoding = Mip { $level } von { $path } wird dekodiert …
image-unsupported = Die Vorschau kann Texturen im Format { $format } nicht dekodieren. Die Tabs Text und Hex zeigen die Daten von { $path } weiterhin.
image-failed = { $path } kann nicht dekodiert werden: { $reason }
image-unshowable = Jedes Mip von { $path } ist größer, als die Vorschau zeigen kann.
container-opening = Wird geöffnet …
container-unreadable = Lässt sich nicht öffnen
container-unreadable-detail = { $name } lässt sich nicht öffnen: { $reason }
container-over-budget = Nicht geöffnet: Speichergrenze
container-over-budget-detail = { $name } wurde nicht geöffnet: Die offenen Container belegen bereits { $limit }. Klappe andere Container zu und klappe ihn erneut auf.
container-stopped = Öffnen abgebrochen
container-stopped-detail = Das Öffnen von { $name } brach ohne Ergebnis ab. Klappe ihn erneut auf, um es noch einmal zu versuchen.
container-archive-changed = Archiv geändert
container-archive-changed-detail = Das Archiv hat sich auf der Festplatte geändert. Lade es neu, um { $name } zu öffnen.
container-status-entries =
    { $count ->
        [one] Container: { $number } Eintrag
       *[other] Container: { $number } Einträge
    }

menu-export = Export
record-export-action = Record-Export …
record-export-title = Records exportieren
record-export-scope-heading = Records
record-export-scope-tree = Im Record-Baum ausgewählt: { $name } ({ $number })
record-export-scope-tree-none = Im Record-Baum ausgewählt (nichts ausgewählt)
record-export-scope-filter = Filter nach Record-Pfad
record-export-filter-hint = Glob mit * oder ?, sonst ein beliebiger Teil des Record-Pfads
record-export-form-heading = Form
record-export-form-json = JSON, eine Datei je Record unter seinem Record-Pfad
record-export-form-ndjson = NDJSON, eine Datei { $name } mit einem Record je Zeile
record-export-form-flat-csv = Werte als CSV, { $name }, eine Zeile je Wert
record-export-form-flat-csv-raw = Werte als CSV, exakt: ohne Byte-Order-Mark, ohne ' vor Text
record-export-form-flat-ndjson = Werte als NDJSON, { $name }, ein Objekt je Wert
record-export-tags = Tag-Pfad neben jeder Tag-Referenz schreiben (nur JSON und NDJSON)
record-export-start = Export starten
record-export-running-title = Exportiere Records
record-export-reading = Lese DataForge …
record-export-selecting = Wähle Records aus …
record-export-progress = { $done } von { $total } Records, { $percent } %
record-export-summary-title = Record-Export abgeschlossen
record-export-summary-cancelled-title = Record-Export abgebrochen
record-export-summary-cancelled = Der Record-Export wurde abgebrochen, bevor alle Records geschrieben waren.
record-export-summary-counts = Records geschrieben { $written }, übersprungen { $skipped }, fehlgeschlagen { $failed }, abgebrochen { $cancelled }
record-export-summary-errors = Records mit Fehlerwerten: { $erroneous }, Fehlerwerte: { $errors }
record-export-summary-no-match = Kein Record passt zum Filter.
record-export-summary-target = Ziel: { $path }
record-export-summary-file-failed = { $path } lässt sich nicht schreiben: { $reason }
record-export-summary-failed = Fehlgeschlagen ({ $number })
record-export-summary-skipped = Übersprungen ({ $number })
record-export-summary-failure = { $path } ({ $guid }): { $reason }
record-export-failed-title = Record-Export fehlgeschlagen
record-export-failed = Der Record-Export nach { $path } ist fehlgeschlagen: { $reason }
record-export-stopped = Der Record-Export nach { $path } brach ohne Ergebnis ab.

catalog-action = Basetool-Katalog …
catalog-file-filter = Basetool-Katalog (JSON)
catalog-file-filter-compressed = Basetool-Katalog (JSON, zstd-komprimiert)
catalog-running-title = Erstelle Basetool-Katalog
catalog-reading = Lese DataForge …
catalog-progress = { $done } von { $total } Records geprüft, { $percent } %
catalog-crafting-progress = Crafting-Daten werden gelesen: { $done } von { $total } Records, { $percent } %
catalog-warning = { $kind ->
    [unreadable_reward_records] { $count } Records, die eine Belohnungsquelle enthalten könnten, lassen sich nicht lesen; ihre Belohnungsquellen fehlen.
    [unknown_modifier_ranges] { $count } Modifikator-Bereiche haben einen Typ, den P4KX nicht kennt; sie fehlen in den Blueprint-Modifikatoren.
    [ambiguous_dismantle] Die DataForge enthält { $count } Zerlegungsprozesse statt einem; keine Zerlegungsausbeute wird verknüpft.
    [unreadable_vehicle_files] Bei { $count } Schiffen lässt sich eine Fahrzeugdatei nicht lesen; ihre Lackierungen fehlen.
    [unplaced_starmap_containers] { $count } Systemcontainer oder Platzierungen lassen sich nicht lesen; die Sternenkarten-Objekte darunter haben keine Position.
    [unreadable_shop_registry] Das Shop-Register lässt sich nicht lesen; der Katalog enthält keine Shops, obwohl der Build welche hat.
    [missing_shop_registry] Das Archiv enthält kein Shop-Register am bekannten Pfad; der Katalog enthält keine Shops.
    [unreadable_shop_containers] { $count } Shop-Container lassen sich nicht lesen; den Shops darin können Art und Marke fehlen.
    [unreadable_shop_inventories] { $count } Inventardateien von Shops lassen sich nicht lesen; die Shops, die sie nutzen, haben keine Artikelliste.
    [missing_shop_inventories] { $count } Inventardateien, die Shop-Container nennen, fehlen im Archiv; die Shops, die sie nutzen, haben keine Artikelliste.
    [no_shop_components] In den .soc-Dateien der Shop-Container wurde keine Shop-Komponente gefunden; kein Shop hat eine Artikelliste.
   *[other] { $message }
}
catalog-warning-more = und { $number } weitere
catalog-writing = Schreibe { $path } …
catalog-summary-title = Basetool-Katalog erstellt
catalog-kinds-heading = Arten
catalog-summary-kind = { $kind ->
    [manufacturers] Hersteller
    [items] Items
    [ships] Schiffe
    [commodities] Waren
    [blueprints] Blaupausen
    [resourceGroups] Ressourcengruppen
    [blueprintCategories] Blaupausenkategorien
    [paints] Lackierungen
    [loadouts] Loadouts
   *[other] { $kind }
}: { $written } geschrieben, { $skipped } übersprungen, { $complete ->
    [yes] vollständig
   *[no] teilweise
}
catalog-summary-loadouts = Loadouts: { $ports } Ports, { $unresolved } nicht aufgelöst, { $docked } angedockte Fahrzeuge, { $outside } Entities ohne Eintrag in Items oder Schiffen
catalog-summary-skipped = Als Entwicklungsreste übersprungene Records: { $number }
catalog-playability-heading = Spielbarkeit
catalog-summary-playability = { $kind ->
    [items] Items
    [ships] Schiffe
   *[other] { $kind }
}: { $player } Spieler, { $ai } KI, { $mission } Mission, { $special } Sonderfall, { $unclassified } nicht eingeordnet
catalog-summary-failed = Nicht lesbare und weggelassene Records ({ $number })
catalog-summary-failure = { $path } ({ $guid }): { $reason }
catalog-texts-heading = Texte
catalog-summary-texts = { $language ->
    [en] Englisch
    [de] Deutsch
   *[other] { $language }
}: { $resolved } aufgelöst, { $missing } fehlen, { $placeholders } Platzhalter
catalog-summary-texts-unavailable = { $language ->
    [en] Englisch
    [de] Deutsch
   *[other] { $language }
}: das Archiv enthält keine global.ini, { $missing } fehlen
catalog-summary-missing = Fehlende Texte: { $number }
catalog-summary-size = Größe: { $size } vom Upload-Limit { $limit }
catalog-summary-size-compressed = Größe: { $compressed } komprimiert (vom Upload-Limit { $limit }), { $size } unkomprimiert
catalog-near-limit = Der Katalog liegt nahe am Upload-Limit des Basetools von { $limit }.
catalog-over-limit = Der Katalog liegt über dem Upload-Limit des Basetools von { $limit }; das Basetool lehnt den Upload ab.
catalog-near-limit-plain = Der JSON-Katalog liegt nahe am Upload-Limit des Basetools von { $limit }. Schreibe den Katalog für den Upload als .json.zst; reines .json dient der Ansicht.
catalog-over-limit-plain = Der JSON-Katalog liegt über dem Upload-Limit des Basetools von { $limit }; das Basetool lehnt ihn ab. Schreibe den Katalog für den Upload als .json.zst; reines .json dient der Ansicht.
catalog-decompressed-near-limit = Der entpackte Katalog liegt nahe am Limit des Basetools von { $limit } für entpackte Inhalte.
catalog-decompressed-over-limit = Der entpackte Katalog liegt über dem Limit des Basetools von { $limit } für entpackte Inhalte; das Basetool lehnt ihn ab.
catalog-summary-file = Datei: { $path }
catalog-cancelled-title = Basetool-Katalog abgebrochen
catalog-cancelled = Der Basetool-Katalog wurde abgebrochen. Es wurde keine Datei geschrieben.
catalog-failed-title = Basetool-Katalog fehlgeschlagen
catalog-failed = Der Basetool-Katalog { $path } ließ sich nicht erstellen: { $reason }
catalog-stopped = Der Basetool-Katalog { $path } brach ohne Ergebnis ab.
catalog-output-is-archive = Der Basetool-Katalog kann nicht nach { $path } geschrieben werden, weil das das geöffnete Archiv ist. Eine andere Datei wählen.
catalog-output-gzip = Der Basetool-Katalog wird nicht nach { $path } geschrieben, weil gzip nicht unterstützt wird. Einen Namen mit der Endung .json.zst für einen komprimierten oder .json für einen einfachen Katalog verwenden.
preview-tab-xml = XML
xml-converting = { $path } wird in XML umgewandelt …
xml-too-large = { $path } ist { $size } groß. CryXml-Einträge über { $limit } werden in der Vorschau nicht umgewandelt; zum Lesen mit Umwandlung extrahieren.
xml-failed = { $path } kann nicht in XML umgewandelt werden: { $reason }
xml-stopped = Das Umwandeln von { $path } brach ohne Ergebnis ab.
xml-truncated = Gekürzt: angezeigt werden die ersten { $lines } Zeilen, die ersten { $limit } des XML.
xml-view = Umgewandeltes XML

scroll-text = Textzeilen
scroll-hex = Hexadezimalzeilen
scroll-image = Bildansicht
scroll-diff = Zeilenunterschiede
scroll-fields = Geänderte Felder
scroll-lines = Record-Felder
scroll-localization = Lokalisierungszeilen
scroll-saved-queries = Gespeicherte Abfragen
scroll-soundbank = Eingebettete WEMs
scroll-summary = DataForge-Übersicht
scroll-list = { $heading }, Zeilen

preview-tab-audio = Audio
audio-codec = Codec
audio-channels = Kanäle
audio-sample-rate = Abtastrate
audio-duration = Dauer
audio-hertz = { $rate } Hz
audio-unknown = –
audio-play = Abspielen
audio-pause = Pause
audio-position = Position
audio-volume = Lautstärke
audio-time = { $position } / { $duration }
audio-decoding = { $path } wird dekodiert …
audio-unsupported = Die Vorschau kann Töne im Codec { $codec } nicht abspielen. Die Tabs Text und Hex zeigen die Daten von { $path } weiterhin.
audio-too-long = { $path } ist zu lang für die Vorschau: der dekodierte Ton wäre größer als { $limit }.
audio-failed = { $path } kann nicht dekodiert werden: { $reason }
audio-output-failed = Der Ton kann nicht abgespielt werden: { $reason }
audio-column-id = ID
audio-prefetched = Vorgeladen
audio-prefetched-note = WEM { $id } ist in dieser Soundbank nur vorgeladen und kann hier nicht abgespielt werden. Der ganze Ton steht im losen WEM-Eintrag mit derselben ID.
audio-plugin = Plug-in
audio-plugin-note = WEM { $id } ist ein Plug-in-Medium, etwa ein Faltungshall-Impuls, und enthält keinen Ton zum Abspielen.
audio-midi = MIDI
audio-midi-note = Das Medium { $id } ist eine MIDI-Datei und enthält keinen Ton zum Abspielen.
audio-bank-summary =
    { $size } · { $count ->
        [one] { $number } WEM
       *[other] { $number } WEMs
    }
audio-bank-empty = { $path } enthält keine WEMs.
audio-bank-failed = Die Soundbank { $path } kann nicht gelesen werden: { $reason }
audio-wem-name = WEM { $id } aus { $bank }
audio-wem-row = WEM { $id }, { $codec }, { $duration }
audio-unsupported-wem = Die Vorschau kann Töne im Codec { $codec } nicht abspielen ({ $name }).

texture-parts =
    { $count ->
        [one] +{ $number } Teil
       *[other] +{ $number } Teile
    }
texture-expand = Teile von { $name } anzeigen
texture-collapse = Teile von { $name } ausblenden

preview-tab-record = Record
record-back = Zurück
record-forward = Vor
record-type = Typ
record-guid = GUID
record-name = Name
record-head-value = Wert
record-loading = Der Record { $path } wird gelesen …
record-failed = Der Record { $path } kann nicht gelesen werden: { $reason }
record-stopped = Das Lesen des Records { $path } brach ohne Ergebnis ab.
record-archive-changed = Das Archiv wurde auf der Festplatte geändert. Neu laden, um den Record zu lesen.
record-dangling = Die Referenz zeigt auf den Record { $guid }, den es in diesem DataForge nicht gibt.
record-follow = Der Referenz auf { $path } folgen
record-row = { $field }: { $value }
record-no-fields = Der Record hat keine Felder.
record-xml-failed = Das Record-XML von { $path } kann nicht geschrieben werden: { $reason }

record-language-menu = Record-Texte
record-language-keys-only = Nur Schlüssel
record-language-unavailable = Das Archiv enthält keine Texte auf { $language }, deshalb stehen nur die Schlüssel da.
record-language-key-row = { $key }: { $text }
record-language-key-row-missing = { $key } (kein Text in dieser Sprache)
record-language-load-failed = Die Texte auf { $language } können nicht gelesen werden, deshalb stehen nur die Schlüssel da: { $reason }
localization-tab = Lokalisierung
localization-label = Suche
localization-hint = Schlüssel oder Text, z. B. item_Name oder Aurora
localization-loading = Die Lokalisierungsdateien werden gelesen …
localization-no-texts = Das Archiv enthält keine global.ini.
localization-failed = Die Lokalisierungsdateien können nicht gelesen werden: { $reason }
localization-stopped = Das Lesen der Lokalisierungsdateien wurde ohne Ergebnis beendet.
localization-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um die Lokalisierungsdateien zu lesen.
localization-empty-title = Kein Archiv geöffnet
localization-empty-hint = Öffnen Sie ein Archiv, um seine Lokalisierungsschlüssel und Texte nachzuschlagen.
localization-column-key = Schlüssel
localization-language-en = Englisch
localization-language-de = Deutsch
localization-language-fr = Französisch
localization-language-it = Italienisch
localization-language-es-ES = Spanisch (Spanien)
localization-language-es-419 = Spanisch (Lateinamerika)
localization-language-pt-BR = Portugiesisch (Brasilien)
localization-language-ja = Japanisch
localization-language-ko = Koreanisch
localization-language-zh-Hans = Chinesisch (vereinfacht)
localization-language-zh-Hant = Chinesisch (traditionell)
localization-language-other = Andere Sprache
localization-missing = Fehlt
localization-keys =
    { $count ->
        [one] { $number } Schlüssel
       *[other] { $number } Schlüssel
    }
localization-keys-shown =
    { $count ->
        [one] { $number } von { $total } Schlüsseln
       *[other] { $number } von { $total } Schlüsseln
    }
localization-searching = Suche läuft …
localization-no-match = Kein Schlüssel und kein Text passt zur Suche.
localization-copy-key = Schlüssel kopieren
localization-copy-text = Text kopieren
localization-notes = Einige global.ini-Dateien wurden nicht sauber gelesen:
tags-tab = Tags
tags-label = Suche
tags-hint = Tag-Pfad, z. B. Weapon.Gun
tags-loading = Die Tags werden gelesen …
tags-no-tags = Das DataForge des Archivs enthält keine Tags.
tags-failed = Die Tags können nicht gelesen werden: { $reason }
tags-stopped = Das Lesen der Tags wurde ohne Ergebnis beendet.
tags-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um die Tags zu lesen.
tags-empty-title = Kein Archiv geöffnet
tags-empty-hint = Öffnen Sie ein Archiv, um seinen Tag-Baum zu durchsuchen.
tags-column-tag = Tag
tags-column-guid = GUID
tags-column-legacy = Legacy-ID
tags-count =
    { $count ->
        [one] { $number } Tag
       *[other] { $number } Tags
    }
tags-count-shown =
    { $count ->
        [one] { $number } von { $total } Tags
       *[other] { $number } von { $total } Tags
    }
tags-no-match = Kein Tag-Pfad passt zur Suche.
tags-copy-path = Tag-Pfad kopieren
tags-copy-guid = GUID kopieren
sounds-tab = Sounds
sounds-entity-label = Entity
sounds-entity-hint = Klassenname, GUID oder Record-Pfad einer Entity
sounds-show = Sounds anzeigen
sounds-use-record = Ausgewählten Record verwenden
sounds-search-label = Suche
sounds-search-hint = Name eines Triggers oder Events, * und ? erlaubt
sounds-export = Sounds exportieren …
sounds-empty-title = Kein Archiv geöffnet
sounds-empty-hint = Öffnen Sie ein Archiv, um die Sounds seiner Entities zu durchsuchen.
sounds-loading = Die Audiodaten des Archivs werden gelesen …
sounds-failed = Die Audiodaten des Archivs können nicht gelesen werden: { $reason }
sounds-stopped = Das Lesen der Audiodaten endete ohne Ergebnis.
sounds-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um seine Audiodaten zu lesen.
sounds-issues =
    { $count ->
        [one] { $number } Audiodatei konnte nicht gelesen werden; die Trigger und Sounds darin fehlen.
       *[other] { $number } Audiodateien konnten nicht gelesen werden; die Trigger und Sounds darin fehlen.
    }
sounds-list-hint = Zeigen Sie eine Entity an oder suchen Sie nach einem Trigger oder Event, um Audio-Trigger aufzulisten.
sounds-list-loading = Die Audio-Trigger werden aufgelistet …
sounds-not-an-entity = Der ausgewählte Record ist keine Entity.
sounds-list-failed = Die Audio-Trigger können nicht aufgelistet werden: { $reason }
sounds-list-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um die Audio-Trigger aufzulisten.
sounds-no-match = Kein Trigger- oder Event-Name passt zur Suche.
sounds-no-triggers = Die Entity nennt keinen Audio-Trigger.
sounds-column-trigger = Trigger
sounds-column-state = Status
sounds-column-events = Wwise-Events
sounds-state-ok = Aufgelöst
sounds-state-no-definition = Ohne Definition
sounds-state-no-event = Ohne Event
sounds-state-no-soundbank = Ohne Soundbank
sounds-count-entity =
    { $count ->
        [one] { $name }: { $number } Audio-Trigger, { $resolved } mit Soundbank
       *[other] { $name }: { $number } Audio-Trigger, { $resolved } mit Soundbank
    }
sounds-count-search =
    { $count ->
        [one] { $number } Trigger
       *[other] { $number } Trigger
    }
sounds-count-search-more = Gezeigt werden die ersten { $number } von { $total } Triggern. Ein längerer Text grenzt die Suche ein.
sounds-scroll-triggers = Audio-Trigger
sounds-scroll-variants = Sounds des Triggers
sounds-select-hint = Wählen Sie einen Trigger, um seine Sounds zu sehen.
sounds-variants-loading = Die Sounds werden aufgelöst …
sounds-undefined = Keine ATL-Definition hat diesen Trigger.
sounds-no-variants = Der Trigger führt zu keinem Sound.
sounds-variants-count =
    { $count ->
        [one] { $name }: { $number } Sound
       *[other] { $name }: { $number } Sounds
    }
sounds-breaks-heading =
    { $count ->
        [one] { $number } Stelle, an der der Weg zu einem Sound abbricht
       *[other] { $number } Stellen, an denen der Weg zu einem Sound abbricht
    }
sounds-break-line = { $name }: { $reason }
sounds-break-line-in = { $name } in { $bank }: { $reason }
sounds-column-event = Wwise-Event
sounds-column-wem = WEM-ID
sounds-column-source = Quelle
sounds-column-language = Sprache
sounds-column-soundbank = Soundbank
sounds-source-embedded = Eingebettet
sounds-source-prefetch = Vorgeladen
sounds-source-streamed = Gestreamt
sounds-source-plugin = Plug-in
sounds-source-external = Extern
sounds-source-midi = MIDI
sounds-source-other = Andere
sounds-language-fallback = { $language } (Ersatz)
sounds-variant-row = { $name }: { $bank }, WEM { $id }
sounds-detail = { $path }, WEM { $id }, { $source }
sounds-select-variant = Wählen Sie einen Sound, um ihn abzuspielen.
sounds-note-prefetch = Von diesem gestreamten Sound ist nur der Anfang eingebettet und der zugehörige WEM-Eintrag fehlt, deshalb lässt er sich nicht abspielen.
sounds-note-plugin = Ein Quell-Plug-in erzeugt diesen Sound; kein WEM enthält ihn.
sounds-note-external = Eine externe Quelle liefert diesen Sound zur Laufzeit; keine Datei enthält ihn.
sounds-note-midi = Dieser Sound ist eine MIDI-Datei und kann hier nicht abgespielt werden.
sounds-note-missing = Weder eine Soundbank noch ein loser WEM-Eintrag enthält dieses WEM.
sounds-note-no-entry = Das Archiv hat keinen Eintrag für die Datei, die diesen Sound enthält, deshalb gibt es keinen Player.
sounds-language-label = Sprache
sounds-export-title = Sounds exportieren
sounds-export-scope = Trigger
sounds-export-selected = Der ausgewählte Trigger: { $name }
sounds-export-selected-none = Der ausgewählte Trigger (keiner ausgewählt)
sounds-export-all =
    { $count ->
        [one] Der Trigger der Liste mit Soundbank
       *[other] Alle { $number } Trigger der Liste mit Soundbank
    }
sounds-export-format = Sounds als
sounds-export-start = Exportieren
sounds-export-running = Sounds werden exportiert
sounds-export-planning = Der Export wird geplant …
sounds-export-progress = { $done } von { $total } Sounds, { $percent } %
sounds-summary-title = Sounds exportiert
sounds-summary-cancelled-title = Sound-Export abgebrochen
sounds-failed-title = Sound-Export fehlgeschlagen
sounds-export-failed = Die Sounds können nicht nach { $path } exportiert werden: { $reason }
sounds-export-stopped = Der Export der Sounds nach { $path } endete ohne Ergebnis.
sounds-summary-cancelled = Der Export wurde abgebrochen. Die noch nicht bearbeiteten Sounds wurden nicht geschrieben.
sounds-summary-counts = Sounds geschrieben { $written }, behalten { $skipped }, fehlgeschlagen { $failed }
sounds-summary-target = Ziel: { $path }
sounds-summary-unconverted =
    { $count ->
        [one] { $number } Sound wurde unverändert geschrieben, weil er sich nicht umwandeln lässt
       *[other] { $number } Sounds wurden unverändert geschrieben, weil sie sich nicht umwandeln lassen
    }
sounds-summary-failed =
    { $count ->
        [one] { $number } Sound wurde nicht exportiert
       *[other] { $number } Sounds wurden nicht exportiert
    }
sounds-summary-file = { $name }: { $reason }
sounds-summary-failure = { $name }, WEM { $id }: { $reason }
sounds-summary-failure-plain = { $name }: { $reason }
tags-irregular =
    { $count ->
        [one] Der Tag-Baum weicht an { $number } Stelle von dem ab, was die TagDatabase auflistet. Der Befehl p4kx dcb tags zählt sie.
       *[other] Der Tag-Baum weicht an { $number } Stellen von dem ab, was die TagDatabase auflistet. Der Befehl p4kx dcb tags zählt sie.
    }
preview-tab-model = Modell
model-reset = Ansicht zurücksetzen
model-hint = Ziehen: drehen · Rechtsklick-Ziehen oder Umschalt-Ziehen: verschieben · Mausrad: zoomen · Pfeile: drehen, Umschalt und Pfeile: verschieben · + und -: zoomen · Pos1: zurücksetzen
model-vertices = Vertices
model-triangles = Dreiecke
model-extent = Größe
model-dimensions = { $width } × { $depth } × { $height } m
model-view = 3D-Ansicht von { $path }: { $vertices } Vertices, { $triangles } Dreiecke
model-loading = { $path } wird gelesen …
model-failed = { $path } kann nicht gelesen werden: { $reason }
model-file-too-large = { $path } oder seine Geometriedatei ist { $size } groß, mehr als die { $limit }, die der Viewer liest.
model-empty = { $path } hat keine Dreiecke zum Anzeigen.
model-too-large = { $path } hat { $vertices } Vertices und { $triangles } Dreiecke, mehr als der Viewer zeigt: { $max_vertices } Vertices und { $max_triangles } Dreiecke.
model-not-finite = { $path } hat keine brauchbaren Koordinaten.
model-gpu-none = Das Fenster hat kein Grafikgerät für den 3D-Viewer. Alles andere funktioniert wie gewohnt.
model-gpu-texture-side = Das Grafikgerät nimmt Texturen mit höchstens { $limit } Pixeln Kantenlänge, zu wenig für den 3D-Viewer. Alles andere funktioniert wie gewohnt.
model-gpu-buffer-size = Das Grafikgerät nimmt Puffer von höchstens { $limit } Byte, zu wenig für den 3D-Viewer. Alles andere funktioniert wie gewohnt.
model-gpu-setup = Das Grafikgerät hat den 3D-Viewer abgelehnt: { $reason } Alles andere funktioniert wie gewohnt.
model-gpu-lost = Das Grafikgerät ging verloren. Starte P4KX neu, um wieder Modelle zu sehen.
model-gpu-failed = Das Grafikgerät konnte das Modell nicht zeichnen. Wähle ein anderes Modell oder starte P4KX neu.
model-gpu-rejected = { $path } ist zu groß für das Grafikgerät.
model-skeleton = Skelett
model-loop = Wiederholen
model-play = Abspielen
model-pause = Pause
model-position = Position in der Animation
model-time = { $position } / { $duration }
model-animations = Animationen
model-bind-pose = Ausgangspose


preview-tab-incoming = Referenziert von
scroll-incoming = Verweisende Records
incoming-head-field = Feld
incoming-head-record = Verweisender Record
incoming-loading = Die Records, die auf { $path } verweisen, werden gesucht …
incoming-failed = Die Records, die auf { $path } verweisen, lassen sich nicht finden: { $reason }
incoming-stopped = Die Suche nach den Records, die auf { $path } verweisen, brach ohne Ergebnis ab.
incoming-archive-changed = Das Archiv wurde auf der Festplatte geändert. Neu laden, um die Records zu finden, die auf einen Record verweisen.
incoming-none = Kein Record verweist auf diesen Record.
incoming-follow = Den Record { $path } öffnen, der in { $field } auf diesen Record verweist
incoming-summary = { $references } von { $records }
incoming-reference-count =
    { $count ->
        [one] { $number } Referenz
       *[other] { $number } Referenzen
    }
incoming-record-count =
    { $count ->
        [one] { $number } Record
       *[other] { $number } Records
    }
incoming-truncated = Die ersten { $shown } von { $total } Referenzen sind aufgelistet.
incoming-incomplete = Einige Records ließen sich nicht lesen oder der Index ist voll, daher kann die Liste Referenzen auslassen.

preview-key-required = { $path } ist verschlüsselt. Gib den Archiv-Schlüssel ein, um es zu lesen.
preview-key-rejected = Der Archiv-Schlüssel entschlüsselt { $path } nicht. Gib den richtigen Schlüssel ein, um es zu lesen.
key-menu = Archiv-Schlüssel…
key-title = Archiv-Schlüssel
key-needed = Ein verschlüsselter Eintrag braucht den Schlüssel dieses Archivs. Gib ihn einmal ein; er wird als Datei in deinem Benutzerprofil gespeichert.
key-rejected = Der verwendete Schlüssel entschlüsselt dieses Archiv nicht. Gib den richtigen Schlüssel ein.
key-intro = Verschlüsselte Einträge lassen sich nur mit dem Schlüssel des Archivs lesen. Gib ihn einmal ein; er wird als Datei in deinem Benutzerprofil gespeichert.
key-label = Schlüssel
key-hint = 32 Hexadezimalziffern
key-show = Schlüssel anzeigen
key-stored = Ein Schlüssel ist gespeichert: { $masked }
key-not-stored = Es ist kein Schlüssel gespeichert.
key-save = Speichern
key-cancel = Abbrechen
key-remove = Gespeicherten Schlüssel entfernen
key-error-length = Ein Schlüssel hat 32 Hexadezimalziffern; eingegeben wurden { $digits }.
key-error-character = Zeichen { $position } ist keine Hexadezimalziffer.
key-error-wrong = Dieser Schlüssel entschlüsselt das Archiv nicht.
key-error-check = Der Schlüssel konnte nicht geprüft werden: { $reason }
key-error-store = Der Schlüssel konnte nicht gespeichert werden: { $reason }
key-error-remove = Der gespeicherte Schlüssel konnte nicht entfernt werden: { $reason }
key-checking = Der Schlüssel wird am Archiv geprüft…
key-note-variable = Die Umgebungsvariable P4KX_ARCHIVE_KEY enthält keinen gültigen Schlüssel und wird ignoriert.
key-note-store = Der gespeicherte Schlüssel kann nicht verwendet werden. Gib den Schlüssel erneut ein, um ihn zu ersetzen. ({ $reason })
about-title = Über P4KX
about-version = Version { $version }
about-copyright = Copyright (c) 2026 the P4KX contributors
about-licence = Lizenziert unter MIT OR Apache-2.0.
about-source = Quellcode:
about-fan-project = P4KX ist ein inoffizielles Star-Citizen-Fanprojekt. Es steht in keiner Verbindung zur Cloud-Imperium-Gruppe und wird nicht von ihr unterstützt oder gebilligt.
about-no-game-data = Es liefert keine Spieldaten mit und liest nur deine eigene Star-Citizen-Installation.
about-responsibility = Du bist dafür verantwortlich, bei der Nutzung die Endbenutzer-Lizenzvereinbarung (EULA) und die Nutzungsbedingungen (Terms of Service) von Star Citizen einzuhalten.
about-owners = Alle Inhalte, die nicht von den Mitwirkenden von P4KX oder seinen Nutzern stammen, sind Eigentum ihrer jeweiligen Inhaber.
about-trademark = Star Citizen®, Roberts Space Industries® and Cloud Imperium® are registered trademarks of Cloud Imperium Rights LLC.
preview-tab-model-info = Info
model-info-geometry = Geometriedatei
model-info-geometry-in-file = In der Modelldatei
model-info-nodes = Knoten
model-info-meshes = Meshes
model-info-lods = Detailstufen
model-info-materials = Materialbibliothek
model-info-material-ids = Material-IDs
model-info-material-ids-none = Keine
model-info-bones = Knochen
model-info-animations = Animationen
model-info-animations-unread = Nicht gelesen
model-info-animations-reading = Wird gelesen …
model-info-chunks = Chunks
model-info-chunks-value = { $total } ({ $read } gelesen, { $skipped } übersprungen, { $unknown } unbekannt)
model-info-materials-unnamed = Keine genannt
model-info-materials-loaded = { $library }, { $count ->
    [one] ein Teilmaterial
   *[other] { $number } Teilmaterialien
  }
model-info-materials-single = { $library }, ein Material für jede ID
model-info-materials-missing = { $library } (existiert nicht)
model-info-materials-unreadable = { $library } (nicht lesbar: { $reason })
model-info-lods-heading = Detailstufen
model-info-column-lod = LOD
model-info-column-vertices = Vertices
model-info-column-triangles = Dreiecke
model-info-column-meshes = Meshes
model-info-chunks-heading = Nicht gelesene Chunks ({ $number })
model-info-column-file = Datei
model-info-column-type = Typ
model-info-column-type-id = Typ-ID
model-info-column-usage = Verwendung
model-info-usage-skipped = Bekannt, nicht gelesen
model-info-usage-unknown = Unbekannt, übersprungen
model-info-file-model = Modelldatei
model-info-file-geometry = Geometriedatei
model-info-file-lod = Geometrie von LOD { $level }
model-info-type-unknown = Unbekannt
model-export-action = Als GLB exportieren …
model-export-title = Als GLB exportieren
model-export-model = Modell: { $name }
model-export-lod-heading = Detailstufe
model-export-lod = LOD { $level }, { $triangles ->
    [one] ein Dreieck
   *[other] { $number } Dreiecke
  }
model-export-materials-heading = Materialien
model-export-materials-colours = Einfache Farben, eine kleine und schnelle Datei
model-export-materials-textures = Mit Texturen, eine größere Datei
model-export-texture-mip-heading = Texturgröße
model-export-texture-mip = { $level ->
    [0] Volle Größe
   *[other] 1/{ $divisor } der Kantenlänge, Mip-Stufe { $level }
  }
model-export-animations-heading = Animationen
model-export-animations = { $count ->
    [one] Die Animation des Modells schreiben
   *[other] Die { $number } Animationen des Modells schreiben
  }
model-export-animations-unknown = Die Animationen des Modells schreiben
model-export-animations-large = Sie werden vollständig gelesen, und die Datei kann groß werden.
model-export-vertex-colours = Vertex-Farben schreiben
model-export-file-heading = Datei
model-export-choose-file = Datei wählen …
model-export-file-none = Keine Datei gewählt
model-export-file-filter = glTF-Binärdateien
model-export-start = Exportieren
model-export-running-title = Modell wird exportiert
model-export-reading = { $name } wird gelesen …
model-export-progress = { $done } von { $total } geschrieben, { $percent } %
model-export-summary-title = Modell exportiert
model-export-summary-cancelled-title = Export abgebrochen
model-export-summary-cancelled = Der Export wurde abgebrochen. Es wurde keine Datei geschrieben.
model-export-summary-file = Geschrieben nach { $path }
model-export-summary-counts = Knoten { $nodes }, Meshes { $meshes }, Materialien { $materials }, Vertices { $vertices }, Dreiecke { $triangles }, Größe { $size }
model-export-summary-bones = Knochen { $bones }, Meshes mit Skinning { $skinned }
model-export-summary-library-missing = Die Materialbibliothek { $library } existiert nicht; es wird das Standardmaterial verwendet.
model-export-summary-library-unreadable = Die Materialbibliothek { $library } ist nicht lesbar ({ $reason }); es wird das Standardmaterial verwendet.
model-export-summary-default-materials = { $count ->
    [one] Eine Material-ID hat kein Teilmaterial in der Bibliothek; dafür wird das Standardmaterial verwendet.
   *[other] { $number } Material-IDs haben kein Teilmaterial in der Bibliothek; dafür wird das Standardmaterial verwendet.
  }
model-export-summary-unnamed-materials = { $count ->
    [one] Ein Eintrag der Materialbibliothek ist kein Material; dafür wird das Standardmaterial verwendet.
   *[other] { $number } Einträge der Materialbibliothek sind kein Material; dafür wird das Standardmaterial verwendet.
  }
model-export-summary-missing-layers = { $count ->
    [one] Eine Layer-Datei der Materialbibliothek fehlt oder ist nicht lesbar; ihre Materialien behalten die Farben der eigenen Datei.
   *[other] { $number } Layer-Dateien der Materialbibliothek fehlen oder sind nicht lesbar; ihre Materialien behalten die Farben der eigenen Datei.
  }
model-export-summary-hidden = { $count ->
    [one] Ein Submesh wurde ausgelassen, weil sein Material nichts zeichnet.
   *[other] { $number } Submeshes wurden ausgelassen, weil ihr Material nichts zeichnet.
  }
model-export-summary-chunks = Nicht exportierte Chunks ({ $number })
model-export-summary-chunk-unknown = Unbekannter Chunk { $index } { $type_id } übersprungen ({ $file })
model-export-summary-chunk-skipped = Chunk { $index } { $type } ({ $type_id }) wird nicht gelesen ({ $file })
model-export-summary-textures = Fehlende Texturen ({ $number })
model-export-summary-texture = { $material }: { $role ->
    [diffuse] Diffuse
    [normal] Normal
    [specular] Specular
   *[other] { $role }
  } { $file } ({ $reason })
model-export-summary-animations = Animationen { $written }, Kanäle { $channels }
model-export-summary-animations-unread = Nicht geschriebene Animationen ({ $number })
model-export-summary-animation-unread = { $name } ({ $source }): { $reason }
model-export-summary-animation-file-unread = { $source }: { $reason }
model-export-summary-animation-unmatched = { $name } ({ $source }): keine Spur passt zu einem Knochen, { $tracks ->
    [0] keine Spur übersprungen
    [one] eine Spur übersprungen
   *[other] { $number } Spuren übersprungen
  }
model-export-failed-title = Export fehlgeschlagen
model-export-failed = { $name } konnte nicht exportiert werden: { $reason }
model-export-no-record = Die DataForge enthält keinen Record { $guid }.
model-export-stopped = Der Export von { $name } wurde ohne Ergebnis beendet.
model-export-key-required = { $name } ist verschlüsselt und das Archiv hat keinen Schlüssel. Gib den Archiv-Schlüssel ein und exportiere erneut.
model-export-key-rejected = Der Archiv-Schlüssel entschlüsselt { $name } nicht. Gib den richtigen Schlüssel ein und exportiere erneut.
model-export-archive-changed = Das Archiv hat sich auf dem Datenträger geändert, deshalb wurde { $name } nicht exportiert. Lade das Archiv neu und exportiere erneut.
scroll-hits = Suchtreffer
content-search-tab = Suche
content-search-label = Suche
content-search-hint = Gesuchter Text, z. B. Gladi
content-search-run = Suchen
content-search-cancel = Abbrechen
content-search-sources = Suchen in
content-search-source-records = Records
content-search-source-localization = Lokalisierung
content-search-source-entries = Einträge
content-search-empty-title = Kein Archiv geöffnet
content-search-empty-hint = Öffnen Sie ein Archiv, um die Inhalte seiner Records, Lokalisierungstexte und Einträge zu durchsuchen.
content-search-prompt = Geben Sie einen Text ein und wählen Sie Suchen. Er wird auch als Teil eines längeren Textes gefunden, unabhängig von der Groß- und Kleinschreibung.
content-search-searching = Suche läuft …
content-search-cancelled = Die Suche wurde abgebrochen.
content-search-failed = Die Suche ist fehlgeschlagen: { $reason }
content-search-stopped = Die Suche wurde ohne Ergebnis beendet.
content-search-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um es zu durchsuchen.
content-search-no-match = Nichts passt zu „{ $text }“.
content-search-none-here = Diese Quelle hat keinen Treffer.
content-search-segment = { $number }
content-search-segment-more = { $number }+
content-search-hits =
    { $count ->
        [one] { $number } Treffer in { $seconds } s
       *[other] { $number } Treffer in { $seconds } s
    }
content-search-more = Es gibt weitere Treffer. Pro Quelle bleiben nur die ersten { $number }; ein längerer Text grenzt die Suche ein.
content-search-skipped =
    { $count ->
        [one] { $number } Eintrag wurde nicht durchsucht: { $binary } binär, { $large } zu groß, { $unreadable } nicht lesbar.
       *[other] { $number } Einträge wurden nicht durchsucht: { $binary } binär, { $large } zu groß, { $unreadable } nicht lesbar.
    }
content-search-key-required =
    { $count ->
        [one] { $number } Eintrag ist verschlüsselt und wurde nicht durchsucht: Das Archiv hat keinen Archivschlüssel oder der Schlüssel passt nicht.
       *[other] { $number } Einträge sind verschlüsselt und wurden nicht durchsucht: Das Archiv hat keinen Archivschlüssel oder der Schlüssel passt nicht.
    }
content-search-key-button = Archivschlüssel eingeben
content-search-no-dataforge = Das Archiv enthält keine DataForge, deshalb wurde kein Record durchsucht.
content-search-column-record = Record-Pfad
content-search-column-field = Feld
content-search-column-text = Text
content-search-column-language = Sprache
content-search-column-key = Schlüssel
content-search-column-entry = Eintrag
content-search-column-line = Zeile
content-search-row = { $place }: { $text }
entities-tab = Entities
entities-label = Suche
entities-hint = Klassenname oder Record-Pfad, z. B. AEGS_Gladius
entities-empty-title = Kein Archiv geöffnet
entities-empty-hint = Öffnen Sie ein Archiv, um ein Schiff, ein Fahrzeug oder eine Waffe zu wählen.
entities-loading = Die Entities werden aufgelistet …
entities-none = Das DataForge des Archivs enthält keine Schiffe, Fahrzeuge oder Waffen.
entities-failed = Die Entities können nicht aufgelistet werden: { $reason }
entities-stopped = Das Auflisten der Entities wurde ohne Ergebnis beendet.
entities-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um die Entities aufzulisten.
entities-filter-all = Alle
entities-filter-ships = Schiffe
entities-filter-vehicles = Fahrzeuge
entities-filter-weapons = Waffen
entities-count =
    { $count ->
        [one] { $number } Entity
       *[other] { $number } Entities
    }
entities-count-shown = { $number } von { $total } Entities
entities-no-match = Keine Entity passt zur Suche.
entities-loadout-heading = Loadout
entities-loadout-none = Wählen Sie eine Entity, um ihr Loadout zu sehen.
entities-loadout-resolving = Das Loadout wird aufgelöst …
entities-loadout-failed = Das Loadout kann nicht aufgelöst werden: { $reason }
entities-loadout-no-record = Das Loadout kann nicht aufgelöst werden: Die DataForge enthält keinen Record { $guid }.
entities-loadout-lost = Das Loadout kann nicht aufgelöst werden: Der Arbeitsthread wurde unerwartet beendet.
entities-loadout-stopped = Das Auflösen des Loadouts wurde ohne Ergebnis beendet.
entities-loadout-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um das Loadout aufzulösen.
entities-loadout-no-ports = Diese Entity hat keine Ports.
entities-loadout-mismatch = { $entity } passt nicht zum Port { $port } und wurde trotzdem eingesetzt: { $reason }
entities-changes = { $excluded } ausgelassen, { $replaced } ersetzt
entities-no-changes = Das Standard-Loadout
entities-reset = Änderungen zurücksetzen
entities-export = Exportieren …
entities-include = { $port } einbeziehen
entities-replace = Ersetzen …
entities-restore = Zurücksetzen
entities-status-excluded = Ausgelassen
entities-status-replaced = Ersetzt
entities-status-docked = Angedocktes Fahrzeug
entities-status-empty = Leer
entities-status-unresolved = Nicht aufgelöst
entities-expand-port = { $name } aufklappen
entities-collapse-port = { $name } zuklappen
entities-variants =
    { $count ->
        [one] { $number } Variante
       *[other] { $number } Varianten
    }
entities-show-variants = Varianten von { $name } anzeigen
entities-hide-variants = Varianten von { $name } ausblenden
entities-base-row = { $name }, { $variants }
entities-variant-row = { $name }, Variante
entities-replace-title = { $port } ersetzen
entities-replace-takes = Der Port nimmt { $types } auf, { $single ->
    [yes] Größe
   *[other] Größen
  } { $sizes }.
entities-replace-takes-types = Der Port nimmt { $types } auf.
entities-replace-takes-sizes = Der Port nimmt { $single ->
    [yes] die Größe
   *[other] die Größen
  } { $sizes } auf.
entities-replace-takes-any = Der Port nimmt jede Entity auf.
entities-replace-hint = Klassenname oder Record-Pfad
entities-replace-count = { $number } Entities, davon { $total } passend
entities-replace-none = Keine Entity passt zur Suche.
entities-replace-misfit = Passt nicht, { $facts }
entities-facts-sized = { $type }, Größe { $size }
entities-facts = { $type }
entities-facts-none = Kein Item-Typ
entities-view-label = 3D-Ansicht von { $name }: { $parts } Teile, { $triangles } Dreiecke
entities-view-lod = Detailgrad
entities-view-materials = Materialien
entities-view-colours = Farben
entities-view-textures = Texturen
entities-view-reading = Modelle werden gelesen … { $count }
entities-view-flattening = Modelle werden vorbereitet … { $done } von { $total }
entities-view-texture-progress = Texturen werden gelesen … { $done } von { $total }
entities-view-interior = Interior
entities-view-interior-progress = Interior wird gelesen … Container { $done } von { $total }, { $models } von { $of } Modellen
entities-view-stop = Abbrechen
entities-view-stopped = Der Aufbau der Ansicht wurde abgebrochen.
entities-view-restart = Erneut aufbauen
entities-view-empty = Kein Teil dieses Loadouts hat ein Modell, das der Viewer zeigen kann.
entities-view-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um das Loadout zu sehen.
entities-view-key = Die Modelle sind verschlüsselt und der Archiv-Schlüssel fehlt oder ist falsch.
entities-view-failed = Die Modelle können nicht gelesen werden: { $reason }
entities-view-parts = Teile
entities-view-meshes = Meshes
entities-view-triangles = Dreiecke
entities-view-show-all = Alle zeigen ({ $count })
entities-view-issues = Nicht aufgelöste Teile: { $count }
entities-hide = Ausblenden
entities-show = Einblenden
entities-status-hidden = Ausgeblendet
assembly-export-title = Entity exportieren
assembly-export-form-heading = Ausgabe
assembly-export-form-glb = GLB-Datei
assembly-export-form-package = Blender-Paket
assembly-export-form-package-note = Ein Ordner mit scene.json, einer GLB-Datei je Mesh und den Texturen als PNG, für das Blender-Add-on.
assembly-export-folder-heading = Paketordner
assembly-export-choose-folder = Ordner wählen …
assembly-export-folder-none = Kein Ordner gewählt
assembly-export-package-replaces = Der Ordner enthält ein früheres Paket. Es wird ersetzt, sobald das neue vollständig ist.
assembly-export-package-not-a-package = Der Ordner ist nicht leer und enthält kein von P4KX geschriebenes Paket. Wähle einen neuen oder einen leeren Ordner.
assembly-export-package-no-name = Der Pfad benennt keinen Ordner. Wähle einen anderen.
assembly-export-running-title-package = Blender-Paket wird exportiert
assembly-export-writing-package = Das Paket von { $name } wird geschrieben …
assembly-export-summary-title-package = Blender-Paket geschrieben
assembly-export-summary-package-counts = Teile { $parts } ({ $modelled } mit Modell), Knoten { $nodes }, Meshes { $meshes }, Texturen { $textures }, Materialien { $materials }, Paletten { $palettes }, Dateien { $files }, Vertices { $vertices }, Dreiecke { $triangles }, Größe { $size }
assembly-export-summary-package-textures = { $count ->
    [one] Eine Textur konnte nicht geschrieben werden; ihre Materialien behalten die einfachen Farben.
   *[other] { $number } Texturen konnten nicht geschrieben werden; ihre Materialien behalten die einfachen Farben.
  }
assembly-export-title-interior = Interior exportieren
assembly-export-action-interior = Interior exportieren …
assembly-export-entity = Entity: { $name }
assembly-export-container = Container: { $name }
assembly-export-interior-heading = Interior
assembly-export-interior = Räume und Props der Objekt-Container einschließen
assembly-export-changes = Loadout: { $excluded } ausgelassen, { $replaced } ersetzt
assembly-export-changes-none = Loadout: das Standard-Loadout
assembly-export-picks-heading = Loadout-Alternativen
assembly-export-picks-hint = Body=1, Eggs=2 oder 1
assembly-export-picks-note = Für Kreaturen und NPCs, bei denen das Spiel je Gruppe zufällig eine Alternative wählt. Gruppe und Index nennen, durch Kommas getrennt; ein Index allein gilt für jede Gruppe, die ihn hat. Die Zählung beginnt bei 0; nicht genannte Gruppen behalten die erste Alternative.
assembly-export-picks-invalid = Jede Wahl als Gruppe=Index schreiben, etwa Body=1, oder als Index allein, durch Kommas getrennt.
assembly-export-lod-heading = Detailstufe
assembly-export-lod = LOD { $level }
assembly-export-materials-heading = Materialien
assembly-export-materials-colours = Einfache Farben, eine kleine und schnelle Datei
assembly-export-materials-textures = Mit Texturen, eine größere Datei
assembly-export-texture-mip-heading = Texturgröße
assembly-export-texture-mip = { $level ->
    [0] Volle Größe
   *[other] 1/{ $divisor }
  }
assembly-export-vertex-colours = Vertex-Farben schreiben
assembly-export-file-heading = Datei
assembly-export-choose-file = Datei wählen …
assembly-export-file-none = Keine Datei gewählt
assembly-export-file-filter = glTF-Binärdateien
assembly-export-start = Exportieren
assembly-export-running-title = Entity wird exportiert
assembly-export-running-title-interior = Interior wird exportiert
assembly-export-resolving = Das Loadout von { $name } wird aufgelöst …
assembly-export-reading = Die Modelle von { $name } werden gelesen …
assembly-export-reading-interior = Das Interior von { $name } wird gelesen … { $done } von { $total } Modelldateien
assembly-export-writing = Die Datei wird geschrieben, bisher { $size } …
assembly-export-summary-title = Entity exportiert
assembly-export-summary-title-interior = Interior exportiert
assembly-export-summary-interior-opened = Interior: geöffnete Container { $containers }, Teile { $parts }, Modelle { $models } ({ $modelled } mit Geometrie), Lichter { $lights }
assembly-export-summary-interior = Interior: Objekt-Container { $containers }, Teile { $parts }, Modelle { $models } ({ $modelled } mit Geometrie), Lichter { $lights }
assembly-export-summary-cancelled-title = Export abgebrochen
assembly-export-summary-cancelled = Der Export wurde abgebrochen. Es wurde keine Datei geschrieben.
assembly-export-summary-file = Geschrieben nach { $path }
assembly-export-summary-mismatch = { $entity } passt nicht zum Port { $port } und wurde trotzdem geschrieben: { $reason }
assembly-export-summary-counts = Teile { $parts } ({ $modelled } mit Modell), Knoten { $nodes }, Meshes { $meshes }, Materialien { $materials }, Vertices { $vertices }, Dreiecke { $triangles }, Größe { $size }
assembly-export-summary-images = { $images ->
    [0] Es werden keine Texturen geschrieben; jedes Material behält seine einfache Farbe.
    [one] Eine Textur wird für { $textured } von { $materials } Materialien geschrieben.
   *[other] { $images } Texturen werden für { $textured } von { $materials } Materialien geschrieben.
  }
assembly-export-summary-fallbacks ={ $count ->
    [one] Einem Modell fehlt die gewählte Detailstufe; es wird in der nächstniedrigeren geschrieben.
   *[other] { $number } Modellen fehlt die gewählte Detailstufe; sie werden in der nächstniedrigeren geschrieben.
  }
assembly-export-summary-hidden = { $count ->
    [one] Ein Submesh wurde weggelassen, weil sein Material nichts zeichnet.
   *[other] { $number } Submeshes wurden weggelassen, weil ihr Material nichts zeichnet.
  }
assembly-export-summary-missing-layers = { $count ->
    [one] Eine Layer-Datei der Materialbibliotheken fehlt oder ist nicht lesbar; ihre Materialien behalten die Farben der eigenen Datei.
   *[other] { $number } Layer-Dateien der Materialbibliotheken fehlen oder sind nicht lesbar; ihre Materialien behalten die Farben der eigenen Datei.
  }
assembly-export-report-heading = Teile, die nicht vollständig aufgelöst werden konnten ({ $number })
assembly-export-report-all = Alle Teile wurden aufgelöst.
assembly-export-report-skipped = Ausgelassen: { $path } ({ $name }): { $reason }
assembly-export-report-note = Hinweis: { $path } ({ $name }): { $reason }
characters-tab = Charaktere
characters-empty-hint = Öffne ein Archiv, um einen Charakter zusammenzustellen.
characters-body-heading = Körper
characters-body-male = Männlich
characters-body-female = Weiblich
characters-body-from-head = Der Kopf bestimmt den Körper.
characters-head-heading = Kopf
characters-head-none = Kein Kopf gewählt.
characters-head-line = { $name }, Kopf-Rig { $rig }
characters-head-reading = Die CHF-Datei wird gelesen …
characters-head-failed = Die CHF-Datei lässt sich nicht verwenden: { $reason }
characters-head-from-archive = Aus dem Archiv wählen …
characters-head-from-file = CHF-Datei öffnen …
characters-head-remove = Kopf entfernen
characters-armour-heading = Rüstung und Kleidung
characters-loadout-none = Kein Loadout gewählt.
characters-loadout-line = Loadout: { $name }
characters-loadout-choose = Loadout wählen …
characters-loadout-clear = Loadout entfernen
characters-pieces-none = Keine Teile hinzugefügt.
characters-piece-add = Teil hinzufügen …
characters-piece-remove = { $name } entfernen
characters-choose-title-piece = Teil hinzufügen
characters-choose-title-loadout = Loadout wählen
characters-choose-title-head = Kopf wählen
characters-choose-hint = Name oder Ordner
characters-choose-count = { $number } von { $total }
characters-choose-none = Nichts passt zur Suche.
characters-expressions-heading = Ausdrücke
characters-expressions-load = Ausdrücke des Kopfes laden
characters-expressions-no-head = Wähle einen Kopf, um seine Ausdrücke zu laden.
characters-expressions-none = Der Kopf hat keine Ausdrücke.
characters-expression-neutral = Neutrales Gesicht
characters-expression-strength = Stärke { $percent } %
characters-export-action = Exportieren …
characters-export-setup-title = Charakter exportieren
characters-export-setup-name = Charakter: { $name }
characters-export-package-note = Ein Ordner mit scene.json und einer GLB-Datei des Charakters: Skelett, Shape Keys und die Materialien, mit ihren Texturen, wenn die Texturen gewählt sind, und die Animationen, wenn du sie wählst, für das Blender-Add-on.
characters-export-animations-heading = Animationen
characters-export-animations-none = Keine
characters-export-animations-all = Alle
characters-export-busy = Der Charakter wird gerade neu aufgebaut. Exportiere ihn, sobald das fertig ist.
characters-export-package-counts = Teile { $parts }, Knochen { $bones }, Skin-Meshes { $skinned }, Shape Keys { $morphs }, Animationen { $animations }, Vertices { $vertices }, Dreiecke { $triangles }, Dateien { $files }, Größe { $size }
characters-export-counts = Teile { $parts }, Knochen { $bones }, Vertices { $vertices }, Dreiecke { $triangles }, Größe { $size }
characters-report-heading = Teile, die nicht vollständig angepasst werden konnten ({ $number })
characters-report-all = Alle Teile passen.
characters-report-skipped = Ausgelassen: { $name }: { $reason }
characters-report-note = Hinweis: { $name }: { $reason }
characters-report-replaced = { $name } des Loadouts wurde durch { $by } ersetzt, das denselben Gegenstandstyp hat.
characters-export-running-title = Charakter wird exportiert
characters-export-done-title = Charakter exportiert
characters-view-preparing = Der Charakter wird vorbereitet …
characters-export-skipped =
    { $count ->
        [one] Ein Teil konnte nicht angepasst werden und fehlt in der Datei.
       *[other] { $number } Teile konnten nicht angepasst werden und fehlen in der Datei.
    }
characters-chf-filter = CHF-Charakterdateien
ui-tab = UI
ui-mode-label = Quelle
ui-mode-files = Dateien
ui-mode-ships = Schiffe
ui-search-label = Suche
ui-files-hint = GFx- oder SWF-Pfad
ui-ships-hint = Klassenname oder Pfad
ui-empty-title = Kein Archiv geöffnet
ui-empty-hint = Öffnen Sie ein Archiv, um seine UI-Dateien und die Bildschirme seiner Schiffe zu zeichnen.
ui-files-loading = Die UI-Dateien werden gesucht …
ui-files-none = Das Archiv enthält keine GFx- oder SWF-Dateien.
ui-files-failed = Die UI-Dateien können nicht aufgelistet werden: { $reason }
ui-files-stopped = Das Auflisten der UI-Dateien wurde ohne Ergebnis beendet.
ui-files-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um die UI-Dateien aufzulisten.
ui-files-count =
    { $count ->
        [one] { $number } Datei
       *[other] { $number } Dateien
    }
ui-files-count-shown = { $number } von { $total } Dateien
ui-ships-none = Das DataForge des Archivs enthält keine Schiffe oder Fahrzeuge.
ui-ships-count =
    { $count ->
        [one] { $number } Schiff oder Fahrzeug
       *[other] { $number } Schiffe und Fahrzeuge
    }
ui-ships-count-shown = { $number } von { $total } Schiffen und Fahrzeugen
ui-no-match = Nichts passt zur Suche.
ui-screens-heading = Bildschirme
ui-screens-pick = Wählen Sie ein Schiff, um seine Bildschirme aufzulisten.
ui-screens-loading = Das Loadout wird aufgelöst …
ui-screens-failed = Die Bildschirme können nicht aufgelistet werden: { $reason }
ui-screens-stopped = Das Auflisten der Bildschirme wurde ohne Ergebnis beendet.
ui-screens-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um die Bildschirme aufzulisten.
ui-screens-none = Dieses Schiff zeigt keine Bildschirme.
ui-screen-own = Das Schiff selbst
ui-screen-size = { $width } × { $height } px
ui-size-label = Größe
ui-size-stage = Bühne
ui-size-wide-1024 = 1024 px breit
ui-size-wide-2048 = 2048 px breit
ui-size-full-hd = 1920 × 1080
ui-background-label = Hintergrund
ui-background-transparent = Transparent
ui-background-movie = Filmfarbe
ui-background-black = Schwarz
ui-background-white = Weiß
ui-power-label = Zustand
ui-power-at-rest = In Ruhe
ui-power-on = Eingeschaltet
ui-variables-label = Variablen
ui-variables-hint = powerstate=1 isradar=1
ui-variables-invalid = Keine Angabe der Form NAME=WERT: { $settings }
ui-format-label = Format
ui-format-png = PNG
ui-format-svg = SVG
ui-export = Exportieren …
ui-export-running = { $file } wird geschrieben …
ui-export-done = { $file } geschrieben: { $width } × { $height } px
ui-export-done-incomplete =
    { $count ->
        [one] { $file } geschrieben: { $width } × { $height } px, { $number } Ding ausgelassen (siehe Reiter „Ausgelassen“)
       *[other] { $file } geschrieben: { $width } × { $height } px, { $number } Dinge ausgelassen (siehe Reiter „Ausgelassen“)
    }
ui-export-failed = Der Export ist fehlgeschlagen: { $reason }
ui-export-stopped = Der Export wurde ohne Ergebnis beendet.
ui-export-is-archive = Die Datei ist das geöffnete Archiv und wird nie überschrieben.
ui-export-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Es wurde nichts exportiert.
ui-export-written-archive-changed = { $file } geschrieben, doch das Archiv hat sich inzwischen auf dem Datenträger geändert. Prüfen Sie die Datei oder exportieren Sie erneut.
ui-export-dismiss = Schließen
ui-filter-png = PNG-Bilder
ui-filter-svg = SVG-Dokumente
ui-rendering = Wird gezeichnet …
ui-render-failed = Das Bild kann nicht gezeichnet werden: { $reason }
ui-render-stopped = Das Zeichnen wurde ohne Ergebnis beendet.
ui-render-archive-changed = Das Archiv hat sich auf dem Datenträger geändert. Laden Sie es neu, um das Bild zu zeichnen.
ui-pick-file = Wählen Sie eine Datei, um ihr Bild zu sehen.
ui-pick-screen = Wählen Sie einen Bildschirm, um sein Bild zu sehen.
ui-too-large = Das Bild ist { $width } × { $height } px groß, zu groß für die Vorschau. Exportieren Sie es stattdessen.
ui-shown = Bild: { $width } × { $height } Pixel
ui-detail-contents = Inhalt
ui-detail-report = Ausgelassen
ui-contents-none = Noch nichts anzuzeigen.
ui-report-none = Alles wurde gezeichnet.
ui-report-more = { $number } weitere Zeilen werden hier nicht gezeigt.
ui-note-lossless = verlustfrei, { $size }
ui-note-embedded = eingebettetes Bild
ui-note-external = externes Bild, { $size }
ui-note-part = Teil einer anderen Bitmap
ui-note-font =
    { $count ->
        [one] { $name }, { $number } Glyphe
       *[other] { $name }, { $number } Glyphen
    }
ui-note-sprite =
    { $count ->
        [one] { $number } Frame
       *[other] { $number } Frames
    }
ui-fact-format = Format
ui-fact-stage = Bühne
ui-fact-frames = Frames
ui-fact-tags = Tags
ui-fact-shapes = Formen
ui-fact-bitmaps = Bitmaps
ui-fact-fonts = Schriften
ui-fact-sprites = Sprites
ui-fact-entity = Entity
ui-fact-port = Port
ui-fact-size = Größe
ui-fact-canvases = Canvases
ui-fact-textures = Texturen
ui-fact-vectors = Vektorgrafiken
ui-fact-movies = Filme
ui-fact-keys = Schlüssel
ui-fact-widgets = Widgets
ui-fact-texts-drawn = Texte gezeichnet
ui-fact-texts-left-out = Texte ausgelassen
ui-kind-shape = Form
ui-kind-morph-shape = Morph-Form
ui-kind-bitmap = Bitmap
ui-kind-font = Schrift
ui-kind-sprite = Sprite
ui-kind-import = Import
ui-kind-canvas = Canvas
ui-kind-texture = Textur
ui-kind-vector = Vektorgrafik
ui-kind-movie = Film
ui-kind-key = Lokalisierungsschlüssel
ui-kind-element = Widgets gezeichnet
ui-kind-element-undrawn = Widgets nicht gezeichnet
ui-report-skipped = Übersprungen
ui-report-effect = Ohne Effekt gezeichnet
ui-report-left-out = Ausgelassen
ui-report-missing-canvas = Canvas nicht gezeichnet
ui-report-missing-texture = Textur nicht im Archiv
ui-report-unknown-variable = Spielvariable, die der Screen nicht liest
ui-report-truncated = Vorzeitig beendet: Ein Limit der Komposition wurde erreicht
ui-report-import = Import nicht gefunden
ui-report-tag = Unlesbarer Tag
ui-report-bitmap = Unlesbare Bitmap
ui-report-warning = Hinweis

folder-conversion-action = Ordner umwandeln …
folder-conversion-running-title = Wandle Ordner um
folder-conversion-progress = { $done } von { $total } Dateien, { $percent } %
folder-conversion-scanning = Lese den Ordner …
folder-conversion-summary-title = Ordner umgewandelt
folder-conversion-summary-cancelled-title = Umwandlung abgebrochen
folder-conversion-summary-counts = Geschrieben { $extracted } ({ $converted } umgewandelt), behalten { $skipped }, fehlgeschlagen { $failed }, abgebrochen { $cancelled }, ohne Umwandlung ausgelassen { $left }
folder-conversion-summary-input = Eingabeordner: { $path }
folder-conversion-failed-title = Umwandlung fehlgeschlagen
folder-conversion-failed = { $path } lässt sich nicht umwandeln: { $reason }
folder-conversion-stopped = Die Umwandlung von { $path } wurde ohne Ergebnis beendet.

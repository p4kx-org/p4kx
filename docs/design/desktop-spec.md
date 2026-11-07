# P4KX · Desktop-Vorgaben des P4KX-Designs

Grundlage: die Tokens des P4KX-Themes (gemessene Palette, `radius-sm` 4 px). Maschinenlesbar:
[`p4kx-desktop-spec.json`](p4kx-desktop-spec.json). Artboards: Design-Canvas „P4KX Desktop".

Alle Maße in px bei 1 rem = 16 px. Farben, Glows und Gewichte als Token-Namen aus der Token-Fixture `crates/p4kx-gui/src/theme/fixtures/tokens.json`.
Die wenigen `rgba(…)`-Werte stehen so im Theme und haben dort kein Token; sie
sind keine neuen Farben.

## Grundregeln

- **Zeilendichte 36 px** für Tabelle, Baum, Channels und Dropdown-Einträge (D.1).
- **Bedienelemente bleiben 44 px** (Buttons, Suchfeld, Tabs, Menü-Links). Kompakte
  DS-Ausnahmen: `.segmented--sm` 34 px, `.btn.btn-xs` 32 px.
- **Schriftgewicht nach Größe:** Text unter 16 px in Regular 400, Light 300 nur für Fließtext ab
  16 px, Bold 700 für Labels, Buttons, Überschriften. Black 900 wird nicht gebraucht,
  die App lädt also drei Fontdateien.
- **Fokusring:** 2 px `color-primary`, Abstand 2 px bei Bedienelementen, −2 px (innen) bei Zeilen
  und Tabs, −3 px bei Segmenten. Auf der CTA weiß.
- **Deaktiviert:** Deckkraft 0,45, kein Hover.
- **Gedrückt:** Das DS kennt keinen Gedrückt-Zustand. Vorgabe hier: CTA `color-accent-dark`,
  Ghost/Outline `control-selected-bg`, Quiet-Danger `rgba(163,0,10,0.2)`, Zeilen wie Hover.
- **Übergänge:** nur Farbe/Deckkraft, 200 ms.
- **Daten-Schreibweise:** Dateinamen, Pfade, Globs und Schnellfilter bleiben unverändert (wie
  `.chip--data`); Versalien nur für Labels, Buttons, Überschriften und Tabellenköpfe.

## Komponententabelle

| Element | DS-Klasse | Höhe | Innenabstand | Abstand | Schrift (px / Gewicht / Tracking) | Versalien | Farben | Rahmen | Glow | Fokus |
| :-- | :-- | --: | :-- | :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| Fenster-Titelleiste | nativ (DWM) | 32 | – | – | System | nein | bg `surface-bar`, Text `color-gray-1` | unten 1 `color-gray-3` | – | System |
| Kopfzeile | App-Header | 46 | x 16 | Marke→Menü 24 | Wortmarke 14,4 / 700 / 0,05 em | ja | bg `surface-bar`, Marke `color-primary` | unten 2 `color-primary` | – | – |
| Menü-Link | – (nav) | 44 | x 14 | 0 | 12,8 / 700 / 0,05 em | ja | `control-fg`; Hover `color-primary`; offen `control-selected-bg` + `control-selected-fg` | offen: Unterstrich 3 `color-primary` | – | innen −2 |
| Dropdown | `.combobox__listbox` | – | y 4 | Untermenü −4 überlappend | – | – | bg `color-gray-4` | 1 `color-primary` | `shadow-dropdown` | – |
| Dropdown-Eintrag | `.combobox__option` | 36 | x 12 | Spalten 12 | 14,4 / 400 / 0; Kürzel 12,8 `color-gray-2-text` | nein | `color-gray-1`; Hover bg `color-gray-3` + `color-primary`; Tastatur bg `color-primary` + `color-bg-black` | Trenner 1 `color-gray-3`, y 4 | – | Tastatur = Füllung |
| Seitenleisten-Titel | `.page-head__eyebrow` | ~32 | 14 / 12 / 6 | – | 11,2 / 700 / 0,15 em | ja | `color-gray-2-text` | – | – | – |
| Channel-Zeile | `.master-row` | 36 | x 12 | Name↔Build 8 | Name 12,8 / 700 / 0,05 em; Build 12,8 / 400 | Namen sind Konstanten | `color-gray-1`, Build `color-gray-2-text`; Hover `rgba(255,255,255,0.03)`; aktiv bg `color-surface-input`, Text `color-white` | unten 1 `color-gray-3`; aktiv links 3 `color-primary` (innen) | – | innen −2 |
| Baumzeile | `.tree-row` / `.tree-name` | 36 | links 8 + 12 je Ebene, rechts 8 | Chevron↔Name 6 | 13,76 / 400; aktiv 700 | nein | wie Channel-Zeile, Hover `rgba(255,255,255,0.025)` (`.tree-row--mid:hover`); Chevron `color-primary` 12 px, Strich 3 | Ebenen-Schiene 1 `color-gray-3` | – | innen −2 |
| Listen-Panel | `.list-panel` / `.master-detail` | füllt | Rand zur Tab-Leiste und zum Fenster 12 (`space-3`), Lücke zwischen den Panels 6 (Größenänderungs-Griff) | – | – | – | bg `surface-panel` | 1 `color-gray-3`, Radius `radius-sm` | – | – |
| Toolbar | `.list-panel__toolbar` | 68 | 12 | 12, bricht um | – | – | bg `surface-panel` | unten 1 `color-gray-3` | – | – |
| Suchfeld | `.list-panel__search input` | 44 | links 38,4, rechts 12 | Basis 260, min. 200 | 16 / 400; Platzhalter kursiv | nein | bg `control-bg`, Text `color-gray-1`, Platzhalter `color-gray-2-text` | 1 `control-border`; Fokus `color-primary` | Fokus `glow-primary` | Rahmen + Glow |
| Schnellfilter | `.segmented--sm` / `__item[aria-pressed]` | 34 | x 12 | Label↔Gruppe 12 | 12,8 / 400 (gewählt 700) / 0 | **nein** | `control-fg` auf `control-bg`; Hover `control-bg-hover` + `color-white`; gewählt `control-selected-bg` + `control-selected-fg` | 1 `control-border`, Trenner 1; gewählt innen 1 + unten 3 `control-selected-border` | – | innen −3 |
| Button-Grundform | `.btn` | 44 (xs 32) | x 16, y 8 (xs 8,8 / 4) | Icon↔Text 8 | 12,8 / 700 / 0,03 em (xs 10,88) | ja | siehe Leiter | Radius `radius-sm` | – | 2 / +2 |
| CTA | `.btn.btn--cta` | 44 | wie oben | – | wie oben | ja | bg `color-primary`, Text `color-bg-black`; Hover `color-accent-light`; gedrückt `color-accent-dark` | – | `glow-primary`, Hover `glow-primary-lg` | weiß, +2 |
| Outline | `.btn-outline` | 44 | wie oben | – | wie oben | ja | `color-primary` auf transparent; Hover `color-accent-light` | 1 `color-primary` | – | 2 / +2 |
| Ghost | `.btn-ghost` | 44 | wie oben | – | wie oben | ja | `control-fg` auf `control-bg`; Hover `color-accent-light` | 1 `control-border`; Hover `color-primary` | – | 2 / +2 |
| Quiet-Danger | `.btn-quiet-danger` | 44 | wie oben | – | wie oben | ja | `color-gray-2-text` auf `control-bg`; Hover `color-danger-text` | 1 `control-border`; Hover `color-danger` | – | 2 / +2 |
| Tabellenkopf | `th` / `.tree-head .tree-cell` | 36 | x 12,8 | – | 11,84 / 700 / 0,07 em | ja | bg `color-surface-input`, `color-gray-1`; sortiert `color-white` | unten 2 `color-primary` | – | innen −2 |
| Sortieranzeige | Chevron (Icon-Set) | 12 | – | Label↔Icon 6 | – | – | aktiv `color-primary`; Hover-Vorschau `color-gray-2-text` | – | – | über Kopfzelle |
| Tabellenzeile | `tbody tr` / `td` | 36 | x 12,8; Checkbox-Spalte 44 | – | 13,76 / 400; aktiv 700 | nein | `color-gray-1`; Zebra `table-stripe-bg` (`color-control` bei 50 %, auf dem Panel `#30353A`); Hover `control-bg`; ausgewählt `control-selected-bg`; aktiv bg `color-surface-input`, Text `color-white` | unten 1 `color-gray-3`; aktiv links 3 `color-primary` (innen) | – | innen −2 |
| Checkbox | `input[type=checkbox]` | 19,2 | – | Trefferfläche = Zelle | Haken | – | bg `control-bg`; an `color-primary`, Haken `color-bg-black`; gemischt: Balken 10 × 2 | 1 `control-border`, Radius 2; Hover `color-primary` | – | `glow-primary`, kein Ring (`input:focus`) |
| Radio | `input[type=radio]` | 19,2 (Zeile 36) | – | Label 12 | Label 14,4 / 400 | nein | wie Checkbox, Punkt 8 `color-bg-black` | 1 `control-border`, rund | – | `glow-primary`, kein Ring (`input:focus`) |
| Tab-Leiste | `.tab-nav` / `.tab` | 44 | Tab x 14, Leiste links 12 | – | 11,52 / 700 / 0,06 em | ja | `color-gray-2-text`; Hover `color-gray-1`; aktiv `color-white` | unten 1 `color-gray-3`; aktiv 3 `color-primary` | – | innen −2 |
| Vorschau-Panel | Fläche + `.tab-nav` | Start 236, min. 120 | Pfadzeile 32, x 16; Inhalt x 16 y 8 | – | Pfad 12,8 / 400 | nein | bg `surface-panel`; Pfad `color-gray-1`, Größe `color-gray-2-text` | 1 `color-gray-3`, Radius `radius-sm` | – | – |
| Hex-Zeile | – | 20 | – | Offset 88; Byte-Zelle 22 + 2; nach 8 Bytes 10; ASCII-Abstand 16, Zelle 10 | 12,8 / 400, Ziffern tabellarisch | Hex in Großbuchstaben | Offset und `00` `color-gray-2-text`; Bytes/ASCII `color-gray-1`; nicht druckbar „." gedämpft | – | – | – |
| Statusleiste | `.footer` (Desktop-Dichte) | 32 | x 16; Einträge x 12 | Trenner 1 `color-gray-3` | 12,8 / 400 | nein | bg `surface-bar`, `color-gray-2-text`; Pfad `color-gray-1`; „Suche …" `color-primary` | oben 1 `color-gray-3` | – | – |
| Leerzustand | `.empty-state` | füllt | 24 | 12 | Titel 13,6 / 700 / 0,06 em; Text 12,8 | Titel ja | bg `color-bg-dark-gray`, Titel `color-gray-1`, Text `color-gray-2-text`; Ziehen: `rgba(231,126,35,0.06)` (`.drop-zone.is-over`) | 1 gestrichelt `color-gray-3`, Radius `radius-sm`; Ziehen `color-primary` | – | Aktion |
| Hinweisleiste | `.alert.alert-warning` | ~44 | x 16, y 10 | 12 | 14,4 / 400 | nein | bg `rgba(255,210,63,0.15)`, Text `color-gray-1`, Icon `color-warning` | 1 `color-warning`, links 4 | – | Aktion |
| Modal | `.modal` (`--wide` 600) | – | Kopf 16 / 12, Körper 16, Fuß 16 / 12 | Gruppen 20, Fuß 8 | Titel 13,6 / 700 / 0,06 em; Körper 14,4 / 400 | Titel ja | bg `color-bg-dark-gray`, Titel `color-white`; Scrim `rgba(0,0,0,0.8)` flach | 1 `color-gray-3`, oben 3 `color-primary`, Ecken 13 × 2 | `glow-primary-lg` | Fokusfalle, Esc |
| Modal Danger | `.modal--danger` | – | wie Modal | – | Titel `color-danger-text` | ja | – | oben 3 + Ecken `color-danger` | `glow-danger-lg` | wie Modal |
| Aufklappbarer Abschnitt | `.panel-header` | 44 | x 16, y 12,8 | Liste max. 180 hoch | 13,6 / 700 / 1 px | ja | bg `color-surface-input`, Titel `color-white`, Chevron `color-primary`; Hover `color-gray-3` | 1 `color-gray-3`, Radius `radius-sm`, links 4 `color-primary` | – | 2 / +2 |
| Fortschrittsbalken | `.attendance-meter` | 8 | – | Text→Balken 12 | Text 14,4 / 400, tabellarisch | nein | Spur `color-surface-input`; Füllung `color-primary`; beim Abbrechen `color-gray-2` | 1 `color-gray-3` | – | – |
| Spinner | `.spinner` | 20 (inline 14) | – | Text 8 | – | – | Spur `rgba(231,126,35,0.25)`, Bogen `color-primary` | Ring 2 | – | – |
| Detailliste | `.kv-list.kv-ruled` | – | unten 8,8 | Zeilen 8,8, Spalten 20; Schlüsselspalte min. 140 | Schlüssel 14,4 / 700 / 0,04 em; Wert 16,8 / 300 | Schlüssel ja | Schlüssel `color-gray-2-text`, Wert `data-fg` | unten 1 `color-gray-3` | – | – |
| Abschnittstitel | `.section-title` | – | unten 8 | oben 32 (als Erstes 0), unten 16 | 16 / 700 / 0,05 em | ja | `color-gray-1` | unten 1 `color-gray-3` | – | – |
| Link | `a` | Text | – | – | 12,8 / 700 | nein | `color-primary`; Hover `color-accent-light` + Unterstrich | – | – | 2 / +2, quadratisch |
| Tooltip | `.scu-hint__bubble` | – | x 9,6, y 8; max. 272 breit | 500 ms Verzögerung | 12,48 / 400, Zeilenhöhe 1,4 | nein | bg `color-control`, `color-gray-1` | 1 `color-primary`, Ecken quadratisch | 0 0 10 0 `rgba(231,126,35,0.3)` | – |
| Scrollbar | `::-webkit-scrollbar` | 12 (Seitenleiste 8) | Griff ohne Einzug | Griff min. 32 | – | – | Spur `color-gray-4`; Griff `color-gray-2`, Hover `color-accent-light`, Ziehen `color-primary` | keine Haarlinie | – | – |
| Größenänderungs-Griff | – | 6 | Griff 2 × 24 mittig | – | – | – | Feld `surface-canvas`; Griff `color-gray-2`; Hover volle Länge `color-accent-light`; Ziehen `color-primary` | – | – | 2 / +2, Pfeiltasten |
| Arbeitsbereich-Tabs | `.tab-nav` / `.tab` | 44 | wie Tab-Leiste | – | wie Tab-Leiste | ja | wie Tab-Leiste | wie Tab-Leiste | – | innen −2 |
| Archiv-Auswahl | `select` + `.combobox__listbox` | 44 | links 12, rechts 32; min. 260 breit | Beschriftung darüber (Eyebrow); Chevron 10,4 vom rechten Rand | 16 / 400; Platzhalter kursiv | nein | bg `control-bg`, Text `color-gray-1`, Platzhalter `color-gray-2-text`; Chevron `color-primary` 12; leere Liste `.combobox__notice` kursiv | 1 `control-border`; Fokus und offen `color-primary` | Fokus und offen `glow-primary` | Rahmen + Glow |
| Faktenleiste | `.facts-bar` | ~32 | x 16, y 8 | 16; Schlüssel→Wert 5 | Schlüssel 9,92 / 700 / 0,08 em; Wert 12,48 / 700 | Schlüssel ja | bg `color-surface-input`, Schlüssel `color-gray-2-text`, Wert `color-white`; Richtungs-Chevron `color-primary` | unten 1 `color-gray-3` | – | – |
| Zähler-Kachel | `.kpi-total.kpi-total--inset` | ~66 | x 14, y 10 | 12 zwischen Kacheln; Chip→Wert 2 | Wert 19,2 / 700 | Chip ja | bg `color-surface-input` (eine Kachel im Panel nimmt nie dessen Füllung), Wert `color-white` | 1 `color-gray-3`, links 4 `color-primary` | – | – |
| Änderungs-Chip | `.chip--success` / `--danger` / `--warning` / `--info` / `--muted` | ~19 | x 8, y 2,4 | – | 11,2 / 700 / 0,06 em | ja | Hinzugefügt: Rand `color-success`, Text `color-success-text`, bg `rgba(35,158,51,0.12)`; Entfernt: `color-danger`, `color-danger-text`, `rgba(163,0,10,0.15)`; Geändert: `color-warning`, `color-warning`, `rgba(255,210,63,0.12)`; Verschoben: `color-info`, `color-info-text`, `rgba(53,93,220,0.15)`; Unlesbar: `color-gray-3`, `color-gray-2-text`, transparent | 1 im Farbton, Radius `radius-sm` | – | – |
| Änderungsliste | Tabellenkopf + Tabellenzeile | 36 | wie Tabelle | Spalten: Änderung 130, Pfad Rest, alte/neue Größe je 150; Verzeichnis-Zähler je 110 | wie Tabelle | Kopf ja | wie Tabelle; fehlende Größe „–" in `color-gray-2-text` | wie Tabelle | – | innen −2 |
| Record-Liste | Tab-Leiste + Tabellenkopf + Tabellenzeile | 36 | wie Tabelle | Spalten: Änderung 130, Record-Pfad Rest, Typ 260 | wie Tabelle | Kopf ja | wie Tabelle; verschoben alter und neuer Pfad mit `chevron-right` 12 `color-primary` dazwischen, Abstand 8 | wie Tabelle | – | innen −2 |
| Feldänderungen | Vorschau-Panel + Tabellenkopf + Tabellenzeile | 36 | Zelle x 12,8 | drei gleiche Spalten Feld, Alt, Neu | wie Tabelle | Kopf ja | Text `color-gray-1`; Zebra und Hover wie Tabelle; fehlend „–" `color-gray-2-text`; Fehlerwert `color-danger-text`; Anzahl `color-gray-2-text` | unten 1 `color-gray-3` | – | innen −2; Tooltip bei Fokus |
| Zeilenvergleich | Vorschau-Panel + Tabellenkopf + Hex-Zeile | Zeile 20 | Zeilennummer x 12,8; Marker→Text 8 | Spalten je Seite halb, Trenner 1 `color-gray-3` | 12,8 / 400, Ziffern tabellarisch | Kopf ja | Text `color-gray-1`, Nummer `color-gray-2-text`; Hinzugefügt bg `rgba(35,158,51,0.12)`, Marker „+" `color-success-text`; Entfernt bg `rgba(163,0,10,0.15)`, Marker „−" `color-danger-text`; Hunk-Kopf bg `color-surface-input`, Text `color-gray-2-text`, aktuell links 3 `color-primary` + `color-white` | – | – | Navigation: Ghost `btn-xs` 2 / +2 |
| Bildvorschau (Tab „Bild") | Vorschau-Panel + `.facts-bar` + `.segmented--sm` | füllt | Inhalt x 16 y 8 | Mip-Label↔Gruppe 12; Mip-Auswahl→Bild 8 | Faktenleiste und Schnellfilter wie oben; Mip-Label wie Eyebrow 11,2 / 700 / 0,15 em | Schlüssel und Label ja, Mip-Werte nein | Bild auf `surface-panel`, 1 Texel = 1 Gerätepixel, zentriert; größere Mips scrollen; fehlende Cubemap-Seiten „–" | wie Faktenleiste | – | Segmente innen −3 |
| Tonvorschau (Tab „Audio") | Vorschau-Panel + `.facts-bar` + `.btn-outline.btn-xs` + `.quality-row`-Range | füllt | Inhalt x 16 y 8 | Bedienelemente 12; Label↔Range 8; Lautstärke 200 breit; Position füllt, min. 200; Zeit so breit wie die breiteste Zeit des Tons | Faktenleiste wie oben; Zeit 12,8 / 400; Lautstärke-Label wie Eyebrow 11,2 / 700 / 0,15 em | Schlüssel, Button und Label ja, Codec und Zeit nein | Zeit `color-gray-1`; unbekannte Dauer „–"; Meldungen `color-danger-text` | wie Faktenleiste | – | Button 2 / +2; Leertaste spielt oder pausiert |
| Modellvorschau (Tab „Modell") | Vorschau-Panel + `.facts-bar` + `.btn-ghost.btn-xs` | füllt, Ansicht min. 96 | Inhalt x 16 y 8; Reset 8 vom Rand | Faktenleiste→Ansicht 8 | Faktenleiste wie oben; Hinweis 12,8 / 400 | Schlüssel und Button ja, Hinweis nein | Ansicht auf `surface-inset`; Hinweis `color-gray-2-text`; Fehler `color-danger-text`; zu groß `color-gray-1` | 1 `color-gray-3`, Ecken quadratisch | – | innen −2; Pfeiltasten, Umschalt, `+` `−`, Pos1 |
| Soundbank-Liste (Tab „Audio") | Vorschau-Panel + Tonvorschau + Tabellenkopf + Tabellenzeile | Kopf und Zeile 36; Panel 373 | wie Tabelle | Spalten: ID 106, Codec 112, Dauer 100 (min. 80), Größe Rest rechtsbündig | wie Tabelle | Kopf ja, Zeilen nein | wie Tabelle; „Vorgeladen", „Plug-in" und fehlende Werte „–" `color-gray-2-text`; Hinweise `color-warning`, Fehler `color-danger-text` | wie Tabelle | – | innen −2; Pfeiltasten, Bild auf/ab, Pos1, Ende |
| Range | `input[type="range"]` (`accent-color: color-primary`), Maße aus `.quality-row` | 18 in einer Zeile von 32 | – | – | – | – | Spur wie Fortschrittsbalken, Füllung `color-primary`; Griff rund 18, `color-primary`, Hover `color-accent-light` | Spur 1 `color-gray-3` | – | 2 / +2; Pfeiltasten, Bild auf/ab, Pos1, Ende |
| Abfrage-Editor | `textarea` | 4 Zeilen (112), wächst bis 12 Zeilen (304), dann Scrollleiste | x 12, y 8 | Label darüber (Eyebrow), Aktionen darunter 12 | 16 / 400, Zeilenhöhe 1,5; Platzhalter kursiv | nein | bg `control-bg`, Text `color-gray-1`, Platzhalter `color-gray-2-text`; Fehlerstelle bg `rgba(163,0,10,0.2)` | 1 `control-border`; Fokus `color-primary`; Fehlerstelle unterstrichen 2 `color-danger` | Fokus `glow-primary` | Rahmen + Glow; Strg+Enter führt aus |
| Fehlerhinweis | `.alert.alert-danger` | ~44 | x 16, y 10 | 12 | 14,4 / 400 | nein | bg `rgba(163,0,10,0.2)`, Text `color-gray-1` | 1 `color-danger`, links 4 | – | – |
| Seitenwahl | `.pagination` / `.page-btn` / `.page-status` | 44 | Knopf x 8, Status x 12 | 6,4 | Status 12,8 / 400, aktuelle Seite 700 `color-primary` | nein | Knopf `control-fg` auf `control-bg`; Hover Rand + Chevron `color-primary`; gesperrt Deckkraft 0,4; Chevron des Icon-Sets 17,6 (1,1 em), Strich 2/24, doppelt mit −6 überlappend | 1 `control-border` | – | 2 / +2 |
| Listenfuß | `.list-panel__footer` | – | x 16, y 12 | Anzahl links, Seitenwahl rechts | 14,4 / 400 | nein | `color-gray-2-text` | oben 1 `color-gray-3` | – | – |
| Abfrageergebnis | Tabellenkopf + Tabellenzeile | 36 | wie Tabelle | Spalten 200 (min. 80), letzte füllt; 500 Zeilen je Seite | wie Tabelle | Record-Pfad-Kopf ja, Pfad-Köpfe nein | wie Tabelle; nicht lesbarer Wert und Liste mit einem solchen `color-danger-text`; `null` `color-gray-2-text`; Zahlenspalten rechtsbündig; Listen als `[a, b, …]`, ab 9 Werten die ersten 8 und „… n more" | wie Tabelle | – | innen −2 |
| Record-Vorschau (Tab „Record") | Vorschau-Panel + `.facts-bar` + Tabellenkopf + Tabellenzeile + `a` | 36 | Zelle x 12,8; 12 je Ebene | zwei gleiche Spalten Feld, Wert; Typ nach dem Link 8 | Werte 13,76 / 400; Link 12,8 / 700 | Kopf und Schlüssel ja, Daten nein | Wert `color-gray-1`, `null` `color-gray-2-text`, Fehler `color-danger-text`; Link `color-primary`, Hover `color-accent-light` + Unterstrich; Ebenen-Schiene 1 `color-gray-3` | unten 1 `color-gray-3` | – | Link 2 / +2; Alt+←, Alt+→, Maustasten zurück/vor |
| XML-Vorschau (Tab „XML") | Vorschau-Panel + Hex-Zeile | Zeile 20 | Inhalt x 16 y 8; Zeilennummer x 12,8 | – | 12,8 / 400, Lato-Standardziffern (ADR-0006) | Tab-Label ja, XML nein | Text `color-gray-1`, Nummer und Escapes `color-gray-2-text`; Kürzungshinweis `color-warning`; Fehler `color-danger-text` | – | – | innen −2; Pfeiltasten, Bild auf/ab, Pos1, Ende |
| Abfrage-Vervollständigung | `.combobox__listbox` / `__option` / `__notice` | Eintrag 36; Liste max. 288 (18 rem), dann Scrollleiste | Eintrag x 12; Hinweis 12 | an der Schreibmarke darunter, ohne Platz darüber; min. 240 breit; Name↔Typ 12 | Name 14,4 / 400, getippter Teil 700 (`<mark>`); Typ 12,8 / 400 rechtsbündig | nein | bg `color-gray-4`; Name `color-gray-1`, Typ `color-gray-2-text`; Hover bg `color-gray-3` + `color-primary`; Tastatur bg `color-primary` + `color-bg-black`; Hinweis kursiv `color-gray-2-text` | 1 `color-primary`; Trenner 1 `color-gray-3` | `shadow-dropdown` | Fokus bleibt im Editor; ↑/↓, Enter/Tab, Esc, Strg+Leertaste |
| Gespeicherte Abfragen | `.panel-header` + Zeile + `.btn-xs` | Kopf 44, Zeile 36; Liste max. 180 | Zeile x 12; Aktionen x 8 | unter den Editor-Aktionen; Name↔Abfrage 8 | Name 12,8 / 700, max. 240 breit mit Auslassung; Abfrage 12,8 / 400, einzeilig, gekürzt | Kopf ja, Namen nein | Name `color-gray-1`, Abfrage `color-gray-2-text`; Hover `color-gray-3`; „Umbenennen" Ghost, „Löschen" Quiet-Danger | unten 1 `color-gray-3` | – | Zeile innen −2; Aktionen 2 / +2 |
| Abfrage speichern / umbenennen / löschen | `.modal` / `.modal--danger` + `.btn-danger` | – | wie Modal | Feld→Hinweis 12; Fuß wie Modal | Feld 16 / 400, Platzhalter kursiv; Hinweis 14,4 / 400 | Titel ja | Hinweis „ersetzt" `color-gray-2-text`, Fehler `color-danger-text` | wie Modal | wie Modal | Fokus im Namensfeld; Enter führt aus, Esc schließt |

## Entscheidungen

**D.1 Zeilendichte: 36 px (Tree-Table-Dichte), keine 44-px-Pflicht für Datenzeilen.**
36 px entsprechen der vorhandenen Dichte `.tree-cell` des Themes (0,5 rem Abstand, 0,86 rem Schrift). Die
wörtliche `td`-Höhe von ~56 px zeigt auf 1440 × 900 nur etwa sieben Zeilen. 34 px wären möglich,
aber Hover, aktive Zeile und Auswahl brauchen etwas Luft, um bei 1 Mio. Zeilen auseinanderzuhalten
zu sein. Die 44-px-Regel ist eine Touch-Regel; P4KX ist ein reines Maus- und
Tastatur-Werkzeug. Datenzeilen sind keine einzelnen Ziele: die ganze Zeile ist klickbar, und die
Checkbox bekommt die volle Zelle (44 × 36) als Trefferfläche. Eigenständige Bedienelemente bleiben
44 px.

**D.2 Einrückung: 12 px je Ebene (`space-3`) plus 1-px-Ebenenschienen.** Zehn Ebenen kosten
120 px, bei 300 px Breite bleiben rund 150 px für den Namen. Mit 24 px wären es 240 px und kaum
noch Platz. Die Schienen (`color-gray-3`) zeigen die Tiefe, damit 12 px reichen. Volle Namen
stehen im Tooltip, die Seitenleiste lässt sich bis 480 px verbreitern.

**D.3 Modal nur, wo eine Entscheidung fällt.** „Einträge extrahieren", die Bestätigung und die
Schriftlizenz sind modal, mit flachem Scrim `rgba(0,0,0,0.8)`. „Extraktion läuft" und
„Extraktion beendet" sind nicht modal und haben keinen Scrim. Während der Extraktion kann man
weiter browsen und die Vorschau nutzen, das Archiv wird nur gelesen. Die drei
Extraktionsaktionen und „Neu laden" sind gesperrt, bis die Extraktion endet.

Ein Startfehler oder unerwartetes Ende des Systemdialogs erscheint als breites Gefahrenmodal
mit der lokalisierten Überschrift „Auswahldialog fehlgeschlagen“, dem tatsächlichen Grund
in einer Gefahrenmeldung und „Schließen“ als einziger Fußaktion. Kopf und Fuß stehen fest;
lange Gründe verwenden den vorhandenen scrollbaren Körper. Ein regulärer Abbruch ist kein
Fehler. Das Modal verwendet ausschließlich die bestehenden Dark-/Lato-Tokens.

**D.4 Marke: Logo-Zeichen vor der Wortmarke, in Orange.** In der Kopfzeile steht das Logo-Zeichen
(Datenklammern um ein X, [`logo.md`](logo.md), ADR-0075) 32 px groß (`brandMarkSize`), 8 px
(`brandGap`) davor die Wortmarke „P4KX" Bold 700, 14,4 px, `color-primary`. Fenster, Taskleiste,
Programmdateien und Installer zeigen die Kachel des Logos.

**D.5 Fortschritt: Orange.** Fortschritt ist laufende Aktivität, wie Spinner und
Lade-Indikator, die das DS orange zeichnet. Grün bedeutet im DS einen erreichten Zustand
(eingecheckt). Neutral wäre auf `color-surface-input` zu schwach. Beim Abbrechen wird die
Füllung `color-gray-2`.

**D.6 Daten bleiben unverändert.** Schnellfilter, Dateinamen, Pfade, Build-Nummern und
Hex-Werte stehen ohne Versalien und ohne Tracking da, wie bei `.chip--data`. Versalien würden
`Game2.dcb` zu einem anderen Suchbegriff machen und Groß- und Kleinschreibung in Pfaden
verstecken.

**D.7 Keine CTA im Hauptfenster.** „Ausgewählte extrahieren …" ist Outline (betonte
Zweitaktion, gesperrt ohne Auswahl), „Angezeigte …" und „Alle …" sind Ghost. Die gefüllte CTA
gehört in den Dialog: „Extrahieren" bzw. „Ganzes Archiv extrahieren". In „Extraktion beendet" ist
„Schließen" die CTA, weil es die einzige Aktion ist und Enter sie auslöst.

**D.8 „Archiv geändert": Warning.** Es ist nichts kaputt, es gibt einen neuen Stand (ein
Launcher-Update). Danger bleibt für echte Fehler („Extraktion fehlgeschlagen").

**D.9 Leerzustand mit Aktion „Archiv öffnen …" als Outline.** Ziehen und Ablegen ist weder per
Tastatur noch per Screenreader erreichbar, deshalb braucht es eine Schaltfläche. Outline statt
CTA, weil der übliche Weg die erkannten Channels links sind.

**D.10 Vergleich als Tab im Arbeitsbereich.** Über der Listenfläche wählt eine `.tab-nav`
zwischen „Durchsuchen" und „Vergleich". Der Vergleich braucht kein geöffnetes Archiv,
deshalb liegt er nicht im Archiv-Menü. Oben stehen die beiden Archiv-Auswahlen (Channels mit
Build, zuletzt geöffnete Archive, „Datei wählen …"), die Ebenen und „Vergleichen" als die eine
CTA des Tabs; „Abbrechen" ist Ghost. Das Ergebnis beginnt mit der Faktenleiste (Alt → Neu, je
Channel und Build), dann drei `.kpi-total`-Kacheln und „Nach Verzeichnis" als eingeklappter
`.panel-header`, darunter die Listenfläche mit Suchfeld, „Bericht speichern …" (Ghost) und der
virtuellen Änderungsliste. Die Verzeichnistabelle startet eingeklappt, damit die Liste bei
800 px Fensterhöhe mehr als ein paar Zeilen zeigt. Der Fortschrittsbalken ist höchstens 320 px breit.

**D.11 Änderungsarten in den semantischen Farben.** Hinzugefügt ist `success`, Entfernt `danger`,
Geändert `warning`, immer als `.chip` mit Text im Text-Tint (Warning ist selbst als Text auf
Schwarz zugelassen). In den Zähler-Kacheln steht der Chip an der Stelle des `.kpi-label`, wie der
Chip in `.kpi-card`; der Akzentbalken bleibt Orange. Die Farbe ist nie der einzige Hinweis, der
Chip trägt immer das Wort.

**D.12 Zeilenvergleich als Vorschau-Panel unter der Änderungsliste.** Sobald eine Änderung
gewählt ist, öffnet sich darunter ein Panel nach dem Muster des Vorschau-Panels, anfangs halb so hoch wie der verbleibende Platz (mindestens 120 px, frei verstellbar); die gewählte Zeile rollt dabei wieder ins Bild. Es zeigt die Pfadzeile mit
Änderungs-Chip, rechts „Vorheriger Unterschied" und „Nächster Unterschied" als Ghost `btn-xs` und die
Position („Unterschied 2 von 5") in `color-gray-2-text`; darunter ein Tabellenkopf „Alt" | „Neu"
und die Zeilen beider Seiten nebeneinander in der Dichte der Hex-Zeile (20 px, 12,8 px). Zeilen
tragen die Füllung und den Text-Tint der Änderungs-Chips aus D.11 (Hinzugefügt `success`,
Entfernt `danger`); Text bleibt `color-gray-1`, damit langer Code lesbar bleibt. Farbe ist nie
der einzige Hinweis: Marker „+" bzw. „−" stehen vor jeder geänderten Zeile, die Zeilennummer bleibt `color-gray-2-text`.
Ein Hunk beginnt mit einer Kopfzeile in `color-surface-input` („Zeilen 10–16" je Seite); der
angesprungene Hunk trägt den aktiven Balken links (3 `color-primary`) wie die aktive Zeile.
Lange Zeilen werden je Seite abgeschnitten und laufen mit einer gemeinsamen horizontalen
Scrollleiste. Binäre und zu große Einträge zeigen statt des Vergleichs den Hinweis im Panel-Körper.

**D.13 Änderungen einer Seite extrahieren: zwei Ghost-Aktionen in der Listen-Toolbar.** „Alt
extrahieren (n) …" und „Neu extrahieren (n) …" stehen links neben der Anzahl und „Bericht
speichern …", als Ghost wie dieses, weil „Vergleichen" die eine CTA des Tabs bleibt (D.7, D.10).
Die Zahl nennt, was extrahiert würde: für Neu die hinzugefügten und geänderten Einträge, für Alt
die geänderten, beide nach dem aktuellen Filter; entfernte Einträge nie. Während ein Filter
läuft, eine Extraktion läuft oder die Zahl 0 ist, sind sie gesperrt (D.3). Lassen sie dem
Suchfeld weniger als seine Basisbreite 260, brechen sie in eine eigene, rechtsbündige Zeile der
Toolbar um. Sie öffnen den vorhandenen Dialog „Einträge extrahieren"; statt der Bereichswahl steht
dort der Bereich als Text, darunter in `color-gray-2-text` der Filter, der ihn einschränkt.
Ziel, Konfliktregel, Umwandlung, Fortschritt, Abbrechen und Zusammenfassung sind die der
normalen Extraktion; ein neuer Ordner trägt Channel und Build der gewählten Seite.

**D.14 Texturen im Tab „Bild".** Wählt man den Header einer Textur (`.dds`, `.dds.a`), öffnet die
Vorschau den Tab „Bild"; andere Einträge kehren zu „Text" zurück. Format, Größe, Mip-Anzahl und
Cubemap-Seiten stehen als `.facts-bar` über dem Bild, wie die Faktenleiste des Vergleichs. Die
Mip-Auswahl ist ein `.segmented--sm` mit der längeren Seite jedes Mips als Daten („2048",
„1024" …): jede Stufe ist eindeutig, einen Klick entfernt und per Tastatur erreichbar. Gezeigt
wird anfangs das größte Mip, das in die Vorschau passt, ein Texel je Gerätepixel ohne Skalierung;
größere Mips scrollen. Mips, die größer sind, als die Vorschau zeigen kann (mehr als
8192 × 8192 Pixel oder über der größten Texturgröße des Renderers), werden nicht angeboten;
hat eine Textur kein solches Mip, sagt der Tab das. Einen Schachbrett-Hintergrund für Transparenz gibt es nicht, weil das DS dafür keine
Farben hat; Alpha wird auf `surface-panel` gemischt.

**D.15 Abfrage als dritter Tab im Arbeitsbereich.** „Abfrage" folgt auf „Durchsuchen" und
„Vergleich". Oben steht der Editor als `textarea` mit „Abfrage ausführen" als der einen CTA des
Tabs; „Abbrechen" ist Ghost, Fortschritt wie beim Vergleich (Spinner, dann Balken bis 320 px). Eine
Abfrage, die nicht läuft, steht als `.alert.alert-danger` unter dem Editor mit Zeile und Spalte,
die betroffene Stelle ist im Editor rot hinterlegt und unterstrichen. Darunter die Listenfläche
mit „CSV/JSON exportieren …" (Ghost, während des Exports „Export abbrechen") und der
Ergebnistabelle; Anzahl und Seitenwahl stehen im `.list-panel__footer` unter der Tabelle, wie in
der Listenfläche des Themes. Die Köpfe der
`select`-Pfade bleiben wie geschrieben (D.6): Feldnamen unterscheiden Groß- und Kleinschreibung,
Versalien würden `Size` und `size` gleich aussehen lassen. Nur der Kopf „Record-Pfad" ist ein
Label in Versalien.

**D.16 Record-Änderungen als zweite Liste mit Feldänderungen im Vorschau-Panel.** Wurde die
Record-Ebene verglichen, steht zwischen Zähler-Kacheln und Listenfläche eine `.tab-nav` mit
„Einträge" und „Records", jeder Tab mit der Zahl seiner angezeigten Zeilen als `.tab-count`
(`color-gray-1`, im aktiven Tab `color-accent-light`, 4 px nach dem Label); nur mit der
Record-Ebene beginnt sie bei „Records". Suchfeld, Anzahl und „Bericht speichern …" bleiben in der
Toolbar, der Filter wirkt auf beide Listen und trifft einen Record über den Record-Pfad einer der
beiden Seiten. Die Aktionen „Alt extrahieren" und „Neu extrahieren" aus D.13 gehören zur
Einträge-Liste: sie extrahieren Einträge, keine Records, und stehen nur, solange „Einträge"
gewählt ist. Die Record-Liste hat die Spalten Änderung (130), Record-Pfad (Rest) und Typ (260),
sortierbar wie die Änderungsliste. Verschoben ist `.chip--info` (Text `color-info-text`), Unlesbar
`.chip--muted`: es ist kein Fehler des Patches, sondern ein Record, der nicht verglichen werden
konnte, und Danger bleibt Entfernt (D.11). Ein verschobener Record zeigt alten und neuen Pfad mit
dem orangen `chevron-right` der Faktenleiste dazwischen; für Screenreader heißt es „alt zu neu".
Passen beide Pfade nicht, enden sie mit Auslassung, der kürzere behält seine Länge. Ein gewählter
Record öffnet darunter das Panel aus D.12: Pfadzeile mit Chip, rechts die Zahl der Änderungen in
`color-gray-2-text`, darunter der Tabellenkopf „Feld" | „Alt" | „Neu" in drei gleich breiten
Spalten und die Feldänderungen als 36-px-Tabellenzeilen mit Zebra und Hover wie die Tabelle, ohne
Auswahl. Jede Zeile ist per Tab erreichbar, trägt dann den Fokusring (innen −2) und zeigt den vollen
Text im Tooltip, wie beim Überfahren. Referenzen stehen als Record-Pfad des Ziels, eine Reference
auf eine Stelle im Record als „Pfad · Stelle"; ein fehlender Wert ist „–" in `color-gray-2-text`,
ein unlesbarer Wert „Fehler: Grund" in `color-danger-text`. Ein geänderter Name oder Typ des
Records steht als eigene Zeile vor den Feldern. Hinzugefügte, entfernte und nur verschobene Records
zeigen statt der Zeilen einen Satz im Panel-Körper, unlesbare den Grund je Seite in
`color-danger-text`. Die Feldänderungen werden erst für den gewählten Record im Hintergrund erneut
verglichen, von einem Arbeits-Thread je Vergleich, der beide DataForges offen hält und nur die
neueste Anfrage beantwortet; die Liste hält nur Pfade und Typen.

**D.17 Export-Menü und Record-Export-Dialog.** Ein eigenes Menü „Export" rechts von „Archiv"
sammelt die Exporte aus DataForge; ein Export ist keine Extraktion von Einträgen (CONTEXT.md),
deshalb steht er nicht im Archiv-Menü. „Record-Export …" ist gesperrt, solange das Archiv kein
DataForge hat oder DataForge nicht geladen werden konnte, und während ein Record-Export läuft.
Der Dialog ist wie „Einträge extrahieren" ein breites Modal (D.3) mit drei Gruppen: Records
(Auswahl im Record-Baum mit Name und Anzahl, oder Filter nach Record-Pfad im Suchfeld-Muster;
Tippen in das Feld wählt den Filter), Form (JSON, NDJSON und die Werte-Formen als Radios, darunter das Kästchen „Tag-Pfad neben
jeder Tag-Referenz schreiben", aus, für die Werte-Formen gesperrt) und Ziel (wie bei der
Extraktion, mit Konfliktregel für einen vorhandenen Ordner). Die eine CTA heißt „Export
starten", nicht „Exportieren", damit sie sich vom Menü „Export" unterscheidet, auch für
Screenreader. Fortschritt und Zusammenfassung sind wie bei der Extraktion nicht modal (D.3, D.7)
und haben je genau eine Aktion im Fuß: „Abbrechen" als Ghost, „Schließen" als CTA. Der Balken
zeigt erst beim Schreiben einen Anteil (D.5); davor steht ein Spinner mit „Lese DataForge …"
bzw. „Wähle Records aus …". Der Hinweis auf einen Abbruch steht in `color-warning` (D.8, D.11).

**D.18 Töne im Tab „Audio".** Wählt man eine WEM (`.wem`), öffnet die Vorschau den Tab „Audio";
andere Einträge kehren zu „Text" zurück. Codec, Kanäle, Abtastrate und Dauer stehen als
`.facts-bar` über den Bedienelementen, wie beim Tab „Bild" (D.14). Darunter eine Zeile:
„Abspielen"/„Pause" als Outline `btn-xs` (D.7: keine CTA im Hauptfenster, Abspielen ist die
betonte Aktion des Tabs), die Zeit („0:03 / 1:12") in `color-gray-1` mit der Breite der breitesten
Zeit des Tons, damit die Position nicht springt, die Position als Range über die restliche Breite
und „Lautstärke" als Eyebrow mit einer Range von 200 px. Die Ranges sind die des Themes:
`input[type="range"]` in der Akzentfarbe, 200 × 18 und 8 px vom Label wie in `.quality-row`
(Specimen MasterDetail). Gefüllt ist bis zum Griff in `color-primary`. Der Griff ist ein Kreis
der Range-Höhe (wie der native Range-Griff), Hover `color-accent-light`. Pfeiltasten gehen 5 s bzw. ein
Zehntel, Bild auf/ab 30 s bzw. ein Viertel, Pos1 und Ende an die Enden. Die Leertaste spielt
oder pausiert, solange kein anderes Bedienelement den Fokus hat. Der Barrierefreiheits-Wert der
Position ist die Zeit. Wechselt der Vorschau-Tab weg von „Audio" oder der Arbeitsbereich weg von
„Durchsuchen", pausiert der Ton an seiner Stelle; „Abspielen" macht dort weiter. Ein Codec ohne
Decoder, ein zu langer Ton und ein Fehler beim Dekodieren oder am Audiogerät stehen als Text in
`color-danger-text` im Panel; Text und Hex bleiben erreichbar.

**D.19 CryXml im Tab „XML".** Ist der gewählte Eintrag CryXml (erkannt an der Signatur, nicht an
der Endung), bietet die Vorschau den Tab „XML" hinter „Hex" an und öffnet ihn, sobald der Anfang
gelesen ist; andere Einträge, auch Text-XML, bekommen den Tab nicht und kehren zu „Text" zurück.
Das umgewandelte XML steht in der Dichte der Hex-Zeile (20 px, 12,8 px, Lato-Standardziffern nach
ADR-0006, weil egui keine tabellarischen Ziffern schalten kann) mit Zeilennummern in
`color-gray-2-text` wie der Zeilenvergleich (D.12), ohne Syntaxfarben, weil das DS keine
Code-Farben kennt. Steuerzeichen erscheinen als Escapes in `color-gray-2-text`, Zeilen enden nach
1024 Zeichen mit Auslassung und laufen mit einer horizontalen Scrollleiste; gezeichnet werden nur
die sichtbaren Zeilen. Mit Tab erhält die Zeilenfläche den Fokus (Ring innen −2) und rollt dann mit
den Pfeiltasten, Bild auf/ab, Pos1 und Ende. Umgewandelt wird im Hintergrund, für Einträge bis
64 MiB; größere zeigen im Tab den Hinweis mit beiden Größen. Einträge über 256 MiB liest die
Vorschau gar nicht (Hinweis im Panel-Körper), sie bekommen deshalb auch keinen Tab „XML". Das XML wird
nach 64 MiB an einer Zeilengrenze gekürzt und trägt dann den Kürzungshinweis in `color-warning`
wie der Tab „Text". Schlägt die Umwandlung fehl, steht der Grund in `color-danger-text` im Tab;
„Text" und „Hex" zeigen die Daten weiter. „Kopieren" kopiert im Tab „XML" das umgewandelte XML.

**D.20 Basetool-Katalog im Export-Menü ohne eigenen Dialog.** „Basetool-Katalog …" und
„Basetool-Diagnosekatalog …" stehen im Menü „Export" unter „Record-Export …". Beide sind gesperrt,
solange das Archiv kein DataForge hat oder DataForge nicht geladen werden konnte, und während ein
Katalog erstellt wird. Jeder Eintrag öffnet direkt den Speichern-Dialog des Systems (D.3), mit
`basetool-catalog.json.zst` bzw. `basetool-catalog.diagnostic.json.zst`; erst die gewählte Datei
startet den Katalog. Die Diagnosefreigabe gilt nur für diesen Auftrag und wird nicht gespeichert.
Normale Exporte verweigern bei unvollständigen Quellen oder Projektionen das Erstellen oder Ersetzen
der Datei; das Gefahrenfenster erläutert die separate Diagnoseaktion. Fortschritts- und Ergebnistitel
benennen einen Diagnosekatalog ausdrücklich. Dessen Ergebnis zeigt zusätzlich einen Hinweis in
`color-warning`. Fortschritt und Ergebnis sind wie beim Record-Export nicht modal mit je genau einer Aktion im Fuß (D.7, D.17):
Spinner beim Lesen von DataForge und beim Schreiben, dazwischen der Balken mit geprüften Records.
Die Zusammenfassung beginnt mit dem Exportzustand: vollständige Quellen und Projektionen in
`color-gray-1`, unvollständige in `color-warning`. Sie gliedert sich mit Gruppenlabels in
„Arten (Auswahlumfang)" (je Art geschrieben, übersprungen,
vollständig oder teilweise, darunter die übersprungenen Entwicklungsreste) und „Texte" (je Sprache
aufgelöst, fehlend, Platzhalter, darunter die fehlenden Texte), dann Größe mit Upload-Limit und
Datei, alles in `color-gray-1`. Größe und Limit stehen in Dezimaleinheiten, das Limit wie in der
Dokumentation als „64 MB". Nahe am Upload-Limit steht oben `.alert-warning`, darüber
`.alert-danger`, weil das Basetool den Upload dann ablehnt. Ein Abbruch schreibt keine Datei und
lässt eine vorhandene unverändert; der Hinweis darauf steht in `color-warning`. Ist die gewählte
Datei das offene Archiv, startet nichts, und das Gefahrenfenster sagt es in `color-danger-text`.
Der Katalog liest DataForge aus dem einen Cache des offenen Archivs (D.22), also nur einmal.

Unter Flatpak ersetzt eine gemeinsame Zielauswahl den Speichern-Dialog für Dateiexporte und
die Zielordnerwahl für Blender-Pakete: Sie zeigt den übergeordneten Ordner, einen einzelnen
Datei- oder Paketordnernamen und den vollständigen Zielpfad. „Ordner wählen …“ öffnet den
Ordnerdialog des Systems für den übergeordneten Ordner; dessen Rückgabepfad bleibt erhalten.
„Export starten“ bestätigt Katalog, Diagnosekatalog, Query, Vergleichsbericht oder UI-Bild und
startet genau diesen Auftrag. Bei Modell-, Entity- und Charakter-Setups übernimmt „Ziel übernehmen“
nur das Ziel in den bestehenden Exportdialog. Ohne gewählten Elternordner oder mit leerem Namen,
Steuerzeichen, absolutem Pfad, Pfadtrennern oder den Komponenten `.` / `..` bleibt die Bestätigung
gesperrt. Die Anzeige enthält dieselbe GLB- und Katalognamensnormalisierung wie der Schreiber.
Die Diagnosefreigabe gilt weiterhin nur für diesen Auftrag; Abbrechen und Besitzerwechsel
verwerfen sie. Verspätete Ordnerantworten einer verworfenen Auswahl werden ignoriert.
Außerhalb Flatpaks bleibt der bisherige Systemdialog bestehen. Die gemeinsame Auswahl verwendet
`modal--wide`, bestehende Feld-, Ghost- und CTA-Widgets, Lato und die vorhandenen Dark-Tokens;
es entstehen keine neuen Maße. Eine Ordnerwahl ersetzt keine Schutz- oder Identitätsprüfung.


Entscheidungen ab **D.22** (Record-Vorschau, Texturgruppen, Scrollflächen, Lokalisierung, Sprache der Record-Texte,
Abfrage-Vervollständigung, Container als Ordner, Tonformat der Extraktion, 3D-Ansicht des Modells, Tags) stehen in
[desktop-spec-decisions-2.md](desktop-spec-decisions-2.md).

## Bewusste Abweichungen von den Web-Specimens

| Abweichung | Grund |
| :-- | :-- |
| Datenzeilen, Baum, Channels, Dropdown-Einträge 36 statt 44 px | Maus/Tastatur-Desktop, Dichte bei 1 Mio. Zeilen (D.1) |
| Baum 12 statt 24 px je Ebene | zehn Ebenen in 300 px (D.2) |
| Text unter 16 px in Regular 400 statt Light 300 | Lesbarkeit kleiner Schrift auf Schwarz |
| Statusleiste 32 statt ~51 px (`.footer` hat 16/32 Innenabstand) | eine Zeile Statusinfo, kein Web-Footer mit Links |
| Kein `text-transform` | egui setzt Versalien beim Zeichnen; der Barrierefreiheits-Name bleibt normal geschrieben |
| Scrim flach statt `backdrop-filter: blur` | egui kann nicht weichzeichnen |
| Scrollbar-Griff volle Breite, keine Haarlinie an der Spur | egui-Scrollbar ohne Einzug; Haarlinie entfällt, weil sie eigene Zeichnung bräuchte |
| Tooltip-Schatten als ein Blur-Schatten 0/0/10/0 | genau ein Schatten je Rechteck |
| Modal ohne Slide, nur 200 ms Deckkraft | nur Farb- und Deckkraft-Übergänge |
| Chevron als gezeichneter Pfad statt `▶` | Lato hat kein `▶`; egui bräuchte eine Fallback-Schrift |
| Hex in festen Zellen statt Monospace | einzige Schrift Lato; die Zellen halten die Spalten, auch für A–F |
| Zeilenvergleich in Lato statt Monospace, Tabs als vier Leerzeichen | einzige Schrift Lato; beide Seiten richten sich an den Zeilen aus, nicht an Spalten |
| Zahlenfeld (Mip-Stufe der Extraktion) als egui-`DragValue` im Look des Eingabefelds: `control-bg`, Rahmen 1 `control-border`, bei Hover und Fokus `color-primary`, Fokus mit `glow-primary`, Höhe 44, Lato 16 / 400 | egui hat kein natives Zahlenfeld; das Design-System gestaltet `input[type=number]` mit den allgemeinen Eingabefeld-Regeln, also wird der `DragValue` nach diesem Vertrag gezeichnet; Ziehen und Tippen, Grenzen 1–15 |
| Checkbox „gemischt" (Balken) ergänzt | DS hat keinen Teilauswahl-Zustand, die Tabelle braucht ihn |
| DataForge-Übersicht, Record-Suche und Feldänderungen des Vergleichs: drei gleich breite Tabellenspalten statt `table-layout: auto` | egui legt Tabellen nicht nach Inhalt aus; gleiche Spalten erfinden keine Breite, lange Werte enden mit Auslassung, Name, Typ und Record-Pfad stehen vollständig im Tooltip der Zeile |
| Abfrageergebnis: Spalten 200 breit (min. 80), verschiebbar, letzte füllt; 500 Zeilen je Seite | egui legt Tabellen nicht nach Inhalt aus, die Zahl der `select`-Spalten ist offen; 200 ist die Breite, bei der Record-Pfade und Werte meist ohne Auslassung passen, 80 hält Kopf und Sortierpfeil lesbar; je Seite 500 Zeilen hält Sortieren und Blättern flüssig |
| Zahlenspalten der Abfrage rechtsbündig ohne tabellarische Ziffern | egui kann OpenType-Merkmale wie `tnum` nicht einschalten; rechtsbündig stehen gleich lange Zahlen trotzdem untereinander |
| Abfrage-Editor 4 Zeilen statt `textarea`-Standardhöhe, Fehlerstelle als Hinterlegung mit Unterstrich | egui kennt keine `rows`-Vorgabe mit eigener Zeilenhöhe; DS hat keine Fehlermarkierung im Text, Farben aus `.alert-danger` |
| Range gezeichnet statt nativ: Spur des Fortschrittsbalkens, runder Griff, Trefferzeile 32 px | egui hat kein `input[type="range"]`; der Griff ist rund wie der native, die Zeile hat die Höhe von `btn-xs`, damit die 18 px hohe Range sich treffen lässt (D.18) |
| Gedrückt-Zustand ergänzt | DS definiert keinen; Werte stammen aus vorhandenen Tokens bzw. Hover-Werten |
| `.panel-header`-Titel 13,6 statt 17,6 px im Dialog | darf nicht größer sein als der Dialogtitel |
| Abfrage-Vervollständigung an der Schreibmarke statt unter dem ganzen Feld | der Editor ist mehrzeilig, ein Name wird mitten im Text ergänzt (D.30) |
| Vervollständigungs-Einträge 14,4 px (`fs-sm`) statt der von `.combobox__option` geerbten 16 px | gleiche Schrift und Zeilenhöhe wie die Dropdown-Einträge (36 px, D.1), damit Name und Typ in eine Zeile passen (D.30) |

## Texte

UI-Texte aus `crates/p4kx-gui/locales`. Neu für die Entwürfe: „Kein Archiv geöffnet"
(Leerzustand-Titel), „Loslassen zum Öffnen" (Ziehen), „Schnellfilter", „Neu laden"-Menüeintrag mit
F5, „Ausgewählte extrahieren …" im Archiv-Menü mit Strg+E, Seitenleisten-Separator „Seitenleiste
verbreitern" / „Vorschau vergrößern" als Barrierefreiheits-Namen. Keine festen Label-Breiten:
Toolbar, Modal-Fuß und Statusleiste brechen um oder kürzen Pfade mit Auslassung.

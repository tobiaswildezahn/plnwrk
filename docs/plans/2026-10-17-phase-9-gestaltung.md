# Phase 9 – Gestaltung und Bedienbarkeit („plnwrk 10×“)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Ziel:** Ein Sachbearbeiter, der plnwrk zum ersten Mal sieht, legt ohne Hilfe einen Vorgang und
ein Projekt an, findet sich in Meilensteinen, Board und Zeitplan sofort zurecht und sagt „das
sieht modern aus, damit arbeite ich gern“. Gemessen wird das an einem Wiederholungstest mit
derselben Person und derselben Aufgabe (Abschnitt 9).

## 1. Warum

- **Realitätstest 4.10 (2026-10-17):** Tobis Sachbearbeiter sollte einen Vorgang und ein
  Projekt anlegen. Ergebnis:
  - „etwas zu unübersichtlich“;
  - die Navigation im Zeitplan hat nicht funktioniert;
  - Design „zu eckig, zu grau, altbacken“.
- **Klick-Durchlauf (Agent, Browser 1280×800):** Die Ausgangslage ist mit 19 Screenshots
  festgehalten in `docs/ux/2026-10-17-ausgangslage/`. Gefunden:
  - Nach dem Schließen eines Dialogs geht der nächste Klick verloren (viermal reproduziert).
  - Im Zeitplan bewegt das Mausrad einer normalen Maus nichts.
  - Es gibt keinen Knopf „Heute“, und beim Zoomen springt die Ansicht an den Projektanfang.
  - Die Schrift ist sehr klein, alles ist grau auf grau, „Löschen“ sieht aus wie „Abbrechen“.
  - Meilensteine sind nur eine Liste.
  - Vorgang oder Projekt wählt man über ein unklares Freitextfeld „Art“.
  - Fehlermeldungen erscheinen in der Statuszeile und halb auf Englisch.
- **Ursache:** plnwrk nutzt fast unverändert das Standard-Aussehen und -Verhalten der
  Bibliothek egui. Das wirkt wie ein Entwicklerwerkzeug. Ein eigenes Gestaltungssystem und
  eine eigene Interaktionsschicht beheben das für alle Ansichten zugleich.

## 2. Entscheidungen Tobi (2026-10-17)

1. Es muss **moderner und schöner** werden: weniger eckig, weniger grau, mit
   **professionell-dezenten Farben**.
2. **Wichtigste Ansichten:** Meilensteine, Kanban-Board, Zeitplan. Dort muss sich UX und
   Design deutlich verbessern.
3. **Ribbons wie in RustGIS** (Office-artige Befehlsleiste).
4. **Termine bleiben im Zeitplan interaktiv änderbar** (Balken ziehen).
5. **Jede Änderung an Meilensteinen braucht eine Bestätigung.**
6. **Bewährte Praktiken einführen:** Maßstab sind die Recherche (Abschnitt 3) und anerkannte
   Gestaltungssysteme.
7. **Die Klickarbeit wird systematisch getestet und durchprobiert** (Abschnitt 9).
8. **Die KI darf Meilensteine ändern, wenn sie vorher im Chat gefragt hat** (Abschnitt 6.3).

## 3. Grundlagen aus der Recherche (Belege im PLAN §9, 2026-10-17)

- **Gantt:**
  - feste Knöpfe „Heute“ und „Ganzes Projekt“;
  - Zoom in Stufen (Tag/Woche/Monat/Quartal);
  - Scrollgrenzen auf den Projektzeitraum plus Puffer;
  - Tabelle links, Diagramm rechts;
  - Abhängigkeiten per Anfasser am Balken ziehen, alternativ per Rechtsklick;
  - Belege: OpenProject, ClickUp, Linear, MS Project; Beschwerden bei monday.com und Zoho.
- **Board:**
  - Karten zeigen Titel, Frist, Person und Fortschritt der Unteraufgaben; alles Weitere per
    Klick (zwei Ebenen höchstens);
  - „+ Aufgabe“ je Spalte;
  - Grenzen der Spaltenbelegung nur als weiche Warnung;
  - Belege: Trello, Planner, Linear, GitHub Projects.
- **Meilensteine:**
  - Rauten auf einer Zeitachse;
  - abgeleitete Ampel statt manuellem Status;
  - Restzeit („noch 12 Tage“);
  - Belege: MS Project Timeline, Asana, Linear.
- **Ribbon (Microsoft-Leitfaden):**
  - höchstens 4 Hauptreiter, „Start“ deckt die häufigen Aufgaben ab;
  - Kontextreiter erscheinen nur bei Auswahl, farbig markiert;
  - jeder Befehl nur an einer Stelle; große Knöpfe vor kleinen;
  - einklappbar;
  - keine Reiter namens „Bearbeiten/Extras/Hilfe“.
- **Gestaltung: KERN UX-Standard** (Verwaltung, IT-Planungsrat 2025):
  - Schrift Fira Sans;
  - Abstandsraster 4/8;
  - Eckenradius 2/4/8;
  - ein Akzent auf warmen, hellen Flächen;
  - Farben nur zweckgebunden (GOV.UK);
  - Kontrast WCAG 2.1 AA: Text 4,5:1, Bedienelemente und Grafik 3:1.
- **RustGIS** (öffentlich, opengeos/RustGIS, Apache-2.0, egui 0.36 wie plnwrk):
  - Ribbon-Bausteine `widgets.rs`, Farbschema `theme.rs`, Logik der Tab-Leiste;
  - eigene Vektor-Symbole statt Emoji;
  - übernehmbar mit Herkunftsvermerk (Commit `72b5829`).

## 4. Ziele, Nicht-Ziele, Anti-Ziele

**Ziele**
- Eigenes Gestaltungssystem (Tokens) für hell und dunkel, in allen Ansichten durchgezogen.
- Ribbon statt Menüleiste, datengetrieben aus dem Befehlsregister.
- Zeitplan, Board und Meilensteine nach den Mustern aus Abschnitt 3 neu gestaltet.
- Die zwei Bedienfehler (verlorener Klick, Mausrad) behoben – sie kommen **zuerst**.
- Formulare: klare Wahl Vorgang/Projekt, Fehler am Feld und auf Deutsch, Enter zum Absenden.
- Bestätigung jeder Meilenstein-Änderung.
- Systematische Klicktests (automatisch und von Hand) und ein Wiederholungstest mit dem
  Sachbearbeiter.

**Nicht-Ziele** (bewusst nicht in dieser Phase)
- Neue Fachfunktionen (Basisplan, Ressourcen, Teamarbeit, Assistent in der App 6b).
- Kalender je Projekt (STATUS 4.13) – erst nach dem Wiederholungstest.
- Kurzbefehl-Tipps im Office-Stil (Alt → Buchstabe), Anpassen der Ribbon durch Nutzer.
- Neues Dateiformat. Ausnahme, falls nötig: Felder nur mit
  `#[serde(default, skip_serializing_if)]`.

**Anti-Ziele** (wäre ein Fehlschlag, selbst wenn es „funktioniert“)
- Bunt, verspielt oder mit viel Bewegung – das widerspricht „professionell-dezent“.
- Die Ribbon frisst den Platz des Zeitplans: auf 13"-Laptops und im Browser muss das
  Diagramm wichtiger bleiben als die Leiste.
- Bedienung nur noch mit der Maus: Tastatur, Screenreader-Namen und die Befehlspalette
  bleiben vollständig.
- Termine verrutschen unbemerkt: Ziehen muss sichtbar sein, ein Rückgängig-Schritt bleiben
  und Meilensteine nie still ändern.
- Desktop und Browser sehen verschieden aus. Gleiche Schrift und gleiche Tokens überall.

## 5. Gestaltungssystem (Phase 9a)

Neues Modul `crates/ui/src/design/` – die **einzige** Quelle für Farben, Abstände, Radien,
Schrift und Symbole. Ansichten verwenden nur noch Tokens, keine festen Farbwerte (ein
Wächtertest wie beim Text prüft das: keine `Color32::from_rgb` außerhalb `design/`).

- **Schrift:** Fira Sans (SIL Open Font License), Schnitte Regular und SemiBold, mitgeliefert
  in App und Browser – das ergibt überall dasselbe Bild.
  - Grundgröße 14 px, klein 12,5 px (nicht kleiner), Überschriften 18/22 px.
  - Ziffern in Tabellen und Zeitachsen mit fester Breite (Fira Sans liefert Tabellenziffern).
  - Die Emoji-Schrift bleibt nur für Inhalte der Nutzer.
- **Abstände:** Raster 4 px. Tokens `xs 4, s 8, m 12, l 16, xl 24, xxl 32`. Zeilenhöhe in
  Listen und Zeitplan 32 px (bisher 24).
- **Radien:** `klein 4` (Eingabefelder, Chips), `mittel 6` (Knöpfe, Balken), `groß 8`
  (Karten, Dialoge). Nicht runder – siehe KERN.
- **Farben (Rolle → hell/dunkel):** Die Werte werden in 9a festgelegt und mit einem
  Kontrasttest (WCAG) belegt. Vorgabe:
  - Hintergrund: warmes Off-White (≈ `#F7F7F5`) bzw. ≈ `#16181C`;
  - Flächen und Karten: Weiß bzw. ≈ `#1F2228`;
  - Linien: hell, etwa 1 Stufe dunkler als die Fläche;
  - Text: ≈ `#1F2630`, gedämpft ≈ `#5B6472`;
  - **ein Akzent:** dunkles Hamburg-Blau ≈ `#1D4F91` (wie das Programmsymbol); Fokus,
    Auswahl und primäre Knöpfe;
  - Statusfarben nur für Status: überfällig/kritisch Rot, gefährdet Bernstein, erledigt Grün,
    Information Blau, jeweils gedämpft;
  - Status nie nur über Farbe: immer zusätzlich Symbol oder Wort.
- **Knöpfe:**
  - primär: gefüllt, Akzent;
  - sekundär: Rahmen;
  - leise: nur Text;
  - **destruktiv: Rot**, nie neben dem primären Knopf ohne Abstand.
  - Normale Knöpfe 32 px hoch. **Kleinste Zielfläche 24 × 24 px** (WCAG 2.2, 2.5.8) – das gilt
    auch für kleine Ribbon-Knöpfe (Symbol 16 px, Fläche 24 px). Abstand zwischen Knöpfen
    ≥ 8 px, in der Ribbon ≥ 4 px.
- **Symbole:** eigene Vektorsymbole (Painter, 24er-Raster, nach dem Vorbild von RustGIS), rund
  30 Stück: Neu, Vorgang, Projekt, Aufgabe, Meilenstein, Rückgängig, Wiederholen, Speichern,
  Öffnen, Export, Heute, Ganzes Projekt, Zoom ±, Filter, Person, Kalender, Abhängigkeit,
  Löschen, Mehr, Board, Liste, Zeitplan, Übersicht, Suche, Einstellungen, Warnung, Erledigt,
  Hilfe. Sie ersetzen die Emoji-Knöpfe (🗙, 📅, 🕑).
- **Bausteine** (`design/widgets.rs`): Knopf (primär/sekundär/leise/destruktiv), Karte, Chip
  (Frist, Status, Person), Fortschrittsbalken (Text **neben** dem Balken), Segmentwahl,
  Feld mit Beschriftung und Fehlertext, Leerzustand (Symbol, Satz, Aktion), Tooltip.
  - Jeder Baustein setzt `widget_info` (Screenreader und kittest-Tests).

## 6. Interaktion und Ribbon

### 6.1 Bedienfehler zuerst (Phase 9a, vor jeder Gestaltung)

1. **Verlorener Klick nach Dialogen:** Ursache finden und beheben, mit Regressionstest
   (kittest: Dialog schließen → nächster Klick wirkt).
   - Vermutung: Ein Modal/Area verbraucht den Zeigerzustand im Bild des Schließens.
   - Betroffen: Löschdialog, Palette, „Aus Vorlage“, „Verwerfen“.
2. **Mausrad im Zeitplan**, alle Wege getestet:
   - Mausrad senkrecht: Zeilen, solange es mehr Zeilen als Platz gibt, sonst die Zeit;
   - Shift+Rad und Trackpad waagerecht: Zeit;
   - Strg/⌘+Rad: Zoom um den Zeiger.

### 6.2 Ribbon (Phase 9b)

- **Ersetzt die Menüleiste.** Die Befehlspalette (⌘K) bleibt. Auf macOS gibt es zusätzlich
  kein natives Menü (egui liefert keins); ⌘Q, ⌘S, ⌘Z usw. bleiben als Kürzel.
- **Reiter:**
  - **Datei:** Bereich hinter der Leiste: Neu, Öffnen, Speichern, Speichern unter, Schützen,
    Export, zuletzt geöffnet.
  - **Start:** Gruppen
    - Neu: Vorgang, Projekt, Aufgabe, Aus Vorlage;
    - Rückgängig/Wiederholen;
    - Status der Auswahl;
    - Suche/Palette.
  - **Ansicht:** Gruppen
    - Ansichten: Übersicht, Liste, Board, Meilensteine, Zeitplan;
    - Navigation: Heute, Ganzes Projekt, Zoomstufe;
    - Darstellung: hell/dunkel, Sprache.
  - **Kontextreiter „Aufgabe“** (farbig, nur bei Auswahl): Status, Zuständig, Termin,
    Meilenstein ja/nein, Wartet auf, Unteraufgabe, Löschen.
  - **Kontextreiter „Zeitplan“** (nur in dieser Ansicht): Heute, Ganzes Projekt,
    Zoom Tag/Woche/Monat/Quartal, Kritischen Pfad zeigen, Als Tabelle exportieren.
- **Maße und Verhalten:**
  - zweizeilig: große Knöpfe mit 32-px-Symbol, kleine mit 16-px-Symbol auf 24-px-Fläche;
  - **einklappbar** über Doppelklick auf einen Reiter oder Strg/⌘+F1, Zustand gemerkt;
  - **Standard: im Browser und unter 900 px Fensterhöhe eingeklappt**; ein Klick auf einen
    Reiter klappt sie vorübergehend auf.
- **Datengetrieben:** `design/ribbon.rs` hält eine statische Tabelle
  `Reiter → Gruppen → Einträge`. Ein Eintrag ist eine Befehls-ID oder eine Oberflächenaktion
  (`Intent`), dazu ein Symbol.
  - Beschriftung, Kurzbefehl, Tooltip und Sperrgrund kommen aus `CommandSpec`
    (`enabled`) und `text.rs`.
  - **Test:** Jede ID steht im Register; jeder Registerbefehl ist in Ribbon, Palette oder
    `palette::ELSEWHERE` erreichbar.
  - Die Palette zeigt zu jedem Treffer seinen Ort („Start › Neu“).
  - Die MCP-Antwort von `search_commands` nennt denselben Ort.
- **Übernahme aus RustGIS:** Bausteine, Tab-Logik (Einklappen, Peek, Scrollpfeile) und
  Farbschema als Vorlage. Kopiert und angepasst, mit Herkunftsvermerk (Apache-2.0, Commit
  `72b5829`) und Eintrag in PLAN §9.

- **Was in der Ansicht bleibt (Entscheidung des Agenten):** Der Leitfaden „jeder Befehl nur an
  einer Stelle“ gilt für Befehle, nicht für Navigation.
  - Die Reiter im Projektkopf (Liste/Board/Meilensteine/Zeitplan) bleiben sichtbar, auch bei
    eingeklappter Ribbon. Der Reiter „Ansicht“ enthält dieselben Ansichten als zweiten Weg,
    so wie Office Zoom im Reiter „Ansicht“ und in der Statusleiste anbietet.
  - Im Zeitplan sitzen **Heute, Ganzes Projekt und die Zoomstufe** als kleine, feste Leiste
    oben rechts im Diagramm, denn die Ribbon kann eingeklappt sein.
  - Der Kontextreiter „Zeitplan“ enthält zusätzlich Export und „Kritischen Pfad zeigen“.
  - Der Abdeckungstest nennt je Befehl einen **Hauptort**; Navigationselemente sind davon
    ausgenommen.

### 6.3 Bestätigung bei Meilensteinen (Phase 9d)

- **Wann:** Jede Änderung an einem Meilenstein fragt vorher nach:
  - Termin (ziehen im Zeitplan oder Meilenstein-Ansicht, Detailbereich);
  - erreicht/wieder offen;
  - Titel;
  - Löschen;
  - Meilenstein-Merkmal an/aus;
  - mittelbar über einen Schub? Nein, Meilensteine werden nie geschoben (bestehende Regel).
- **Inhalt des Dialogs:** was sich ändert (alt → neu) und welche Folgen absehbar sind,
  z. B. „2 Aufgaben enden danach“ oder „Projektfrist überschritten“. Dazu die Knöpfe
  **Ändern** (primär) und **Abbrechen**.
- **Wie gebaut:** ein **zentraler Wächter** in `PlnwrkApp::apply`: Jeder `Intent`, der einen
  Meilenstein ändern würde, wird zu `Dialog::ConfirmMilestone { intent, alt, neu }`, statt
  ausgeführt zu werden. Erst „Ändern“ führt ihn aus. Damit kann keine Ansicht die Bestätigung
  vergessen, und ein Test prüft das für alle Wege.
  - Ein getippter Titel oder ein getipptes Datum bleibt bis dahin Entwurf.
  - **Beim Speichern oder Schließen mit offenem Meilenstein-Entwurf** (`flush_draft`) kommt
    zuerst die Bestätigung. Bei „Abbrechen“ wird der Entwurf verworfen, angezeigt und **nicht**
    gespeichert. Das Speichern selbst läuft weiter; still verloren geht nichts.
- **Ausgenommen (Entscheidung des Agenten):**
  - Rückgängig und Wiederholen (⌘Z/⌘⇧Z): Sie sind selbst das Sicherheitsnetz, eine Rückfrage
    würde sie entwerten.
  - Das Anlegen eines neuen Meilensteins (auch aus Vorlagen).
  - Das Löschen eines ganzen Projekts: Es hat schon eine eigene Rückfrage, die die Zahl der
    Meilensteine nennt.
- **MCP (Entscheidung Tobi, 2026-10-17): Die KI darf Meilensteine ändern, wenn sie vorher im
  Chat gefragt hat.**
  - Umsetzung: `run_command`/`move_task` lehnen jede Meilenstein-Änderung ab, solange
    `"confirmed": true` fehlt.
  - Die Ablehnung, die Werkzeugbeschreibung und die `instructions` des Servers sagen der KI
    wörtlich: erst den Menschen im Chat fragen, was sich ändert (alt → neu, Folgen), und nur
    nach seinem ausdrücklichen Ja mit `"confirmed": true` wiederholen.
  - Die Antwort nennt die Änderung, damit sie im Chat sichtbar bleibt; der Verlauf markiert sie
    wie jede KI-Änderung als „durch KI“.
  - Grenze, bewusst so akzeptiert: Ob die KI wirklich gefragt hat, kann der Server nicht prüfen.
    Er erzwingt nur, dass sie es ausdrücklich erklärt. Dasselbe gilt für `plnwrk-cli run`.

## 7. Die drei Kernansichten

### 7.1 Zeitplan (Phase 9c)

- **Aufbau:**
  - Tabelle links, Spalten Titel, Beginn, Frist, Zuständig; Breite verschiebbar, Spalten
    abwählbar. **Nur lesend** in 9c (Entscheidung des Agenten): Termine ändert man durch
    Ziehen oder im Detailbereich, ein Klick in die Zeile wählt aus. Bearbeiten direkt in der
    Tabelle wie in MS Project kommt frühestens nach dem Wiederholungstest;
  - Diagramm rechts, Zeilenhöhe 32 px;
  - Unteraufgaben ein- und ausklappbar.
- **Navigation:**
  - feste Knöpfe **Heute** und **Ganzes Projekt** (auch im Kontextreiter);
  - Zoomstufen **Tag/Woche/Monat/Quartal** statt −/+, Zoom um den Zeiger bzw. die Mitte;
  - Scrollgrenzen auf Projektzeitraum ± 4 Wochen;
  - **Leere Fläche ziehen = Ansicht verschieben** (Hand-Zeiger);
  - Scrollleisten immer sichtbar und nie über Zeilen;
  - Auswahl in Liste oder Details scrollt die Zeile in Sicht.
- **Balken:**
  - abgerundet, Fortschritt als dunklere Füllung;
  - **Überfällig** mit rotem Rand und Symbol;
  - kritischer Pfad rot (abschaltbar), erledigt gedämpft;
  - Rauten für Meilensteine mit Beschriftung;
  - **an den Enden ziehen = Beginn/Frist ändern**, in der Mitte ziehen = verschieben;
  - Vorschau mit Datum am Zeiger („Fr 16.10. – Di 20.10.“) und Hinweis auf mitrutschende
    Nachfolger („+2 Aufgaben verschieben sich“).
- **Abhängigkeiten:**
  - Anfasser-Punkt am Balkenende bei Hover, auf die Zielaufgabe ziehen;
  - Rechtsklick → „Vorgänger hinzufügen…/Nachfolger hinzufügen…“;
  - Linien gedämpft; bei Auswahl einer Aufgabe ihre Linien hervorgehoben.
- **Sprache:** „Puffer“ → „Spielraum: 39 Tage“ (Tooltip erklärt das Wort).
- **Legende:** nur, wenn etwas Rotes zu sehen ist.
- **Leerer Zeitplan:** Erklärung plus Knopf „Termine vergeben“, der zur Liste führt.

### 7.2 Board (Phase 9e)

- **Spalten:** füllen die Fensterbreite (min. 220 px, sonst waagerecht scrollen mit sichtbarer
  Leiste). Kopf mit Name, Anzahl und „+“ für eine neue Aufgabe in diesem Status.
- **Karte:**
  - Titel (2 Zeilen max.);
  - Fristchip farbig nach Lage (überfällig / heute / bald / später);
  - Personenkreis mit Kürzel;
  - Fortschrittsbalken der Unteraufgaben; die Unteraufgaben einklappbar in der Karte (wie
    seit PR #12);
  - Priorität als Symbol;
  - Meilensteine als Raute im Titel.
- **Ziehen:**
  - Platzhalter zeigt die Ziellücke;
  - **weiche Warnung** beim Ziehen nach „Erledigt“, wenn Vorgänger offen sind;
  - bestehende Rückfrage zu offenen Unteraufgaben bleibt;
  - Meilenstein-Karten fragen nach 6.3.
- **Filter oben:** Zuständig, nur Überfälliges, Suche im Titel.
- **Spaltengrenzen** (optional je Spalte, nur Warnfarbe): **aufgeschoben** auf nach den
  Wiederholungstest.

### 7.3 Meilensteine (Phase 9d)

- **Oben ein Zeitstrahl:**
  - alle Meilensteine des Projekts als Rauten auf einer Zeitachse;
  - Heute-Markierung und Projektfrist;
  - Klick wählt aus, Ziehen ändert den Termin – nach Bestätigung.
- **Darunter Karten**, je Meilenstein:
  - Titel, Datum (mit Uhrzeit);
  - **„noch 12 Tage“ / „seit 3 Tagen überfällig“ / „erreicht am …“**;
  - **abgeleitete Ampel**, abgeleitet aus Zeitplan-Analyse und Status, nicht von Hand
    (Entscheidung des Agenten):
    - **erreicht:** Status erledigt;
    - **überfällig:** Termin vorbei und nicht erreicht;
    - **gefährdet:** ein Vorgänger ist überfällig, oder der Spielraum **bis zu diesem
      Meilenstein** ist ≤ 2 Tage. Gemeint ist der vorhandene Meilenstein-Puffer aus
      `plnwrk-schedule` (frühestmöglich laut Vorgängern gegen den eigenen Termin), nicht der
      Puffer zum Projektende;
    - **im Plan:** sonst;
  - Anzahl offener Vorgänger (aufklappbar als Liste);
  - Zuständig.
- **Aktionen:** „Erreicht“ (Bestätigung), Termin ändern (Bestätigung), Löschen (rot,
  Bestätigung). Der Papierkorb steht nicht mehr direkt am Namen.
- **Übernahme in den Zeitplan:** Die Meilensteine erscheinen dort ebenfalls als Rauten in einer
  eigenen, angehefteten obersten Zeile.

## 8. Übrige Ansichten (Phase 9f)

- **Anlegen-Dialog:**
  - **Segmentwahl Vorgang | Projekt** (Pflicht, Vorauswahl je nach Knopf) mit je einem Satz
    Erklärung und Beispiel;
  - **Kein neues Feld (Entscheidung des Agenten):** Die Wahl schreibt das vorhandene Feld
    `kind`, und zwar „Vorgang“ bzw. „Projekt“. In den Details heißt dieses Feld weiter „Art“
    und lässt sich verfeinern, etwa „SKA“ oder „Beschaffung“.
    - **Regel:** Ein Projekt ist, was die Art „Projekt“/„Project“ oder keine Art trägt und kein
      Aktenzeichen hat. **Alles andere ist ein Vorgang.**
    - Dadurch bleibt eine verfeinerte Art („SKA“) ein Vorgang, die Vorlage „SKA beantworten“
      erzeugt Vorgänge, und vorhandene Dateien gelten ohne Umwandlung.
    - Die Regel steht an **einer** Stelle (`plnwrk-engine`, z. B. `Project::is_case()`) und wird
      überall benutzt: Seitenleiste, Übersicht, Dialog, Export.
  - „Aktenzeichen“ nur bei Vorgang;
  - Fehler **am Feld, auf Deutsch**; „Anlegen“ gesperrt, bis ein Titel steht;
  - **Enter = Anlegen**;
  - Datumswähler bleibt im Dialog;
  - „Person anlegen…“ direkt im Auswahlfeld;
  - „Aus Vorlage“ gesperrt mit Erklärung, wenn es keine Vorlage gibt.
- **Start:**
  - „Beispiel öffnen“ und „Neue Datei“ als zwei klare Karten mit Satz;
  - zuletzt geöffnete Dateien;
  - VS-NfD-Hinweis in die Fußzeile.
- **Übersicht:**
  - Block **„Heute und überfällig“** mit Abhaken;
  - Projektkarten breiter, mit Ampel wie 7.3;
  - Seitenleiste nach **Vorgänge / Projekte / Vorlagen** gruppiert, mit Symbolen.
- **Liste:**
  - Spalten mit Kopfzeile (Frist, Zuständig, Status);
  - „+ Unteraufgabe“ erst bei Hover;
  - neue Aufgabe kurz hervorgehoben.
- **Detailbereich:**
  - Gruppen „Termin“, „Zuordnung“, „Abhängigkeiten“, „Notizen & Verlauf“;
  - **Löschen rot und unten abgesetzt**;
  - Optionen für „Wartet auf“, die einen Kreis ergäben, ausgegraut mit Begründung statt
    unsichtbar;
  - Terminkonflikt markiert.
- **Zeitplan: angeheftete Datumsleiste** (Tobi, 2026-10-17: „ja, Datumsleiste anheften in 9f“):
  - Die Zeitachse (Kalenderwochen, Monate, Tagesziffern) und die Spaltenköpfe der Tabelle
    bleiben stehen, wenn die Zeilen scrollen, wie eine fixierte Zeile in Excel;
  - die Meilenstein-Zeile aus 9d sitzt direkt darunter und bleibt ebenfalls stehen;
  - Heute- und Projektfrist-Beschriftung wandern mit in die angeheftete Leiste (sie ragen bisher
    in die erste Zeile).
- **Leerzustände überall:** Symbol, ein Satz, eine Aktion.
- **Statuszeile:** nur noch für Speichern/Wiederherstellung; Fehler gehören an den Ort.

## 9. Klicktests und Wiederholungstest (Phase 9g, laufend ab 9a)

1. **Automatische Klickpfade (kittest)** je Kernaufgabe als „Erstnutzer-Szenario“, mit Zahl
   der Klicks als Obergrenze:
   - Vorgang anlegen (≤ 5 Klicks + Tippen);
   - Projekt anlegen (≤ 4);
   - Aufgabe mit Frist (≤ 4);
   - im Zeitplan zu heute springen (1);
   - Balken um eine Woche verschieben;
   - Meilenstein verschieben mit Bestätigung;
   - Karte nach Erledigt;
   - Abhängigkeit ziehen.

   Wird ein Pfad länger oder bricht er, schlägt die Schranke an.
2. **Durchlauf von Hand im echten Browser** am Ende jeder Teilphase, nach fester Checkliste:
   - Ablauf: dieselben Aufgaben mit Maus, Trackpad und nur Tastatur, bei 1280×800 und
     1920×1080, hell und dunkel;
   - Screenshots je Ansicht neben die Ausgangslage in `docs/ux/` – das ergibt einen
     Vorher/Nachher-Vergleich.
3. **Kontrast- und Größentest:** Ein Test prüft alle Farbpaare der Tokens gegen WCAG
   (4,5:1 / 3:1) und die Mindestgrößen von Schrift und Zielflächen.
4. **Wiederholungstest mit dem Sachbearbeiter** nach 9f, gleiche Aufgabe wie am 17.10.
   - Er denkt laut, Tobi notiert nur.
   - Gemessen werden Erfolg ja/nein, Zeit, Stockstellen sowie zwei Fragen: „Ist es
     übersichtlich?“ und „Wie wirkt es?“ (je Note 1–5).
   - Ziel: Aufgabe ohne Hilfe gelöst, beide Noten ≥ 4.
   - Ergebnis in PLAN §9.

## 10. Reihenfolge und Lieferung

Je Teilphase gilt: eigener Branch und PR, Review und Nachreview durch unabhängige Agenten,
Merge nach Tobis „ja, merge“. Jede Teilphase ist für sich nutzbar.

| Teil | Inhalt | Ergebnis für Tobi |
| --- | --- | --- |
| **9a** | Bedienfehler (Klick, Mausrad) · Gestaltungssystem: Tokens, Fira Sans, Knöpfe, Symbole, Bausteine · Kontrasttest | App fühlt sich sofort ruhiger, moderner und zuverlässig an |
| **9b** | Ribbon (datengetrieben, einklappbar, Kontextreiter) ersetzt Menüleiste · Palette zeigt Ort | Befehle sind auffindbar wie in Office |
| **9c** | Zeitplan neu (Navigation, Tabelle links, Balken-Ränder, Abhängigkeiten ziehen, Vorschau) | Zeitplan ist bedienbar und sieht gut aus |
| **9d** | Meilenstein-Ansicht (Zeitstrahl, Ampel, Countdown) · Bestätigung jeder Meilenstein-Änderung (App + MCP) | Meilensteine als Management-Sicht |
| **9e** | Board neu (Spaltenbreite, Karten, Filter, Warnungen) | Kanban wie in Planner/Trello |
| **9f** | Dialoge, Start, Übersicht, Liste, Detailbereich, Leerzustände | Erster Eindruck stimmt |
| **9g** | Klickpfad-Szenarien vervollständigen · Durchlauf von Hand · Wiederholungstest · Version 0.2.0 | Messbarer Beleg „10× besser“ |

## 11. Was dem Builder überlassen ist

- Genaue Farbwerte innerhalb der Vorgaben, solange der Kontrasttest besteht.
- Symbolzeichnung im Einzelnen, Animationen nur dezent (≤ 150 ms, abschaltbar über die
  Systemeinstellung „Bewegung reduzieren“, wo verfügbar).
- Interne Aufteilung von `design/` und Ansichten.
- Ob RustGIS-Code kopiert oder neu geschrieben wird – Hauptsache gleiche Bedienlogik.

## 12. Risiken

- **Fira Sans** vergrößert App und Browser-Paket um ≈ 0,5–1 MB. Das ist vertretbar.
- **Ribbon und Platz:** Gegenmittel sind die Standards in 6.2 (eingeklappt im Browser und auf
  kleinen Bildschirmen).
- **egui-Grenzen:**
  - Selbst gezeichnete Bausteine brauchen `widget_info`, sonst sehen Tests und Screenreader
    sie nicht.
  - Barrierefreiheit im **Browser** (AccessKit im Web) ist begrenzt. Prüfen; falls nicht
    machbar, dokumentieren (BITV-Relevanz, § 10 HmbGGbM).
- **Umfang:** sieben Teilphasen. Wenn der Wiederholungstest nach 9c schon deutlich besser
  ausfällt, kann Tobi die Reihenfolge der übrigen Teile neu setzen.

## Demo für Tobi (nach 9g)

1. https://plnwrk.wildezahn.de öffnen bzw. Version 0.2.0 installieren.
2. „Neue Datei“ → Ribbon **Start → Vorgang** → Titel, Aktenzeichen, Frist → Enter.
3. **Ansicht → Zeitplan:** „Heute“, „Ganzes Projekt“, mit dem Mausrad scrollen, mit
   Strg/⌘ zoomen, einen Balken am Ende länger ziehen, eine Abhängigkeit ziehen.
4. **Meilensteine:** Zeitstrahl, Ampel, „noch X Tage“. Einen Meilenstein verschieben – die
   Bestätigung kommt.
5. **Board:** Karte mit „+“ anlegen, nach „Erledigt“ ziehen.
6. Hell/dunkel umschalten; die Screenshots in `docs/ux/` mit der Ausgangslage vergleichen.

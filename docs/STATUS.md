# plnwrk – Stand und offene Punkte

> Übergabedokument für die nächste Session (Mensch oder Agent). Stand: **2026-10-14 abends:
> Phase 5 (Zeitplanung) umgesetzt, Review-Befunde in Arbeit** (Branch `phase-5`, PR #9 offen);
> Phase 4 (Web) ist gemergt. Verbindlich bleibt [PLAN.md](PLAN.md) (Entscheidungsprotokoll §9);
> dieses Dokument fasst den Ist-Zustand und alles Offene zusammen. Nach jeder Phase aktualisieren.

## 1. Kurzfassung

plnwrk („Planwerk“) ist eine Projekt- und Vorgangsmanagement-Software komplett in Rust, nach
dem Vorbild RustGIS: Desktop + Browser, Datenhoheit (eine Datei je Arbeitsbereich), offline,
voll KI-steuerbar, Fernziel VS-NfD. Projekt und Vorgang sind **ein** Datenmodell; Meilensteine
sind Aufgaben mit Merkmal `milestone`.

| Phase | Stand | PR |
| --- | --- | --- |
| 0 Fundament | **fertig** | [#1](https://github.com/tobiaswildezahn/plnwrk/pull/1) |
| 1 Kern (Modell, Befehle, Abfragen, Undo/Redo, CLI) | **fertig**, 2 Reviews | [#2](https://github.com/tobiaswildezahn/plnwrk/pull/2) |
| 2 Speicherung (`.plnwrk`, Verschlüsselung, Export, CLI auf Dateien) | **fertig**, 2 Security-Reviews | [#3](https://github.com/tobiaswildezahn/plnwrk/pull/3) |
| 3a Desktop-MVP (Dateien, Übersicht, Liste, Board, Details, Vorlagen, Personen) | **fertig**, von Tobi getestet | [#4](https://github.com/tobiaswildezahn/plnwrk/pull/4) |
| 3b-1 Datumsauswahl, Fristen mit Uhrzeit, Meilensteine = Aufgaben (Format 2) | **fertig** | [#5](https://github.com/tobiaswildezahn/plnwrk/pull/5) |
| 3b-2 Autosave/Wiederherstellung, Umbenennen in der Liste, Schutz aus der App, Screenreader-Namen | **fertig** | [#6](https://github.com/tobiaswildezahn/plnwrk/pull/6) |
| 3b-3 Zustandsautomat-Test Autosave, Befehlspalette, Englisch, Hell/Dunkel | **fertig**, Review + Nachreview + Kurzprüfung | [#7](https://github.com/tobiaswildezahn/plnwrk/pull/7) |
| 4 Web (Upload/Download, Zwischenstand im Browser, VS-NfD-Sperre) | **fertig**, Review + 3 Nachprüfungen | [#8](https://github.com/tobiaswildezahn/plnwrk/pull/8) |
| 5 Zeitplanung (Arbeitstage Hamburg, Nachfolger schieben, kritischer Pfad, Gantt) | **umgesetzt**, Review: 7 Befunde offen (§3.1) | [#9](https://github.com/tobiaswildezahn/plnwrk/pull/9) |
| 6 KI (MCP, Assistent) | offen | – |
| 7 VS-Härtung | offen | – |
| 8 Veröffentlichung | offen | – |

Alle Phasen ab 3a: Review + Nachreview durch unabhängigen Agenten, Befunde eingearbeitet,
Mutationsproben, CI grün auf Linux, macOS, Windows und Web.

## 2. Technischer Ist-Zustand

- **Repo:** `~/projekte/plnwrk/`, GitHub privat `tobiaswildezahn/plnwrk`. `main` enthält
  Phase 0–4 (Merge `21305ec`). Neue Phase immer auf eigenem Branch (`phase-…`).
- **Toolchain:** Rust 1.99.0 (rustup, `rust-toolchain.toml`), Werkzeuge per Homebrew:
  cargo-deny, trunk, cargo-llvm-cov. Neue Shell: `source ~/.cargo/env` falls `cargo` fehlt.
- **Schranke:** `cargo xtask ci` – Format, Schichtregeln, Reinheit core/engine, Clippy nativ +
  wasm32 mit `forbid`-Lints, WASM-Build, alle Tests, Abdeckung core+engine+store ≥ 90 % (aktuell
  ≈ 94 %), cargo-deny. Letzter Lauf: grün.
- **CI:** GitHub Actions. Linux, macOS und Web-Bundle bei jedem Push (1–3 min); **Windows
  (≈ 15–25 min) nur vor dem Merge**: PR-Label `merge` setzen (`gh pr edit <nr> --add-label
  merge`), grün abwarten, dann `gh pr merge <nr> --squash --match-head-commit <sha>`.
  Direktes Pushen auf `main` ist für Agenten gesperrt (Freigabe-Regel); Doku-Nachträge gehen über
  den nächsten Phasen-Branch oder Tobi pusht selbst.
- **Dateiformat:** Version 2 (`manifest::VERSION`). Version 1 wird gelesen; echte v1-Dateien
  liegen als Testmaterial in `crates/store/tests/fixtures/` (im Repo, `.gitignore`-Ausnahme).
- **Crates:** `core` (Modell, Invarianten, Rang, Policy, Uhr-Trait), `engine` (Befehle, Events,
  Undo/Redo, Abfragen, Fixture, `workspace.upgrade`), `store` (Dateiformat, Krypto,
  Passphrasen-Regel, Export, Dateien, `Document::open_session`), `platform` (echte Uhr,
  Zufalls-IDs), `ui` (egui-Oberfläche inkl. Autosave), `schedule`/`assistant`/`mcp` (Gerüste);
  Apps `plnwrk` (Desktop, Systemdialoge, ohne macOS-Standardmenü), `plnwrk-web` (dieselbe
  Oberfläche im Browser, noch ohne Dateien), `plnwrk-cli`.
- **Tests:** u. a. 99 Oberflächentests ohne Fenster, ein Zustandsautomat-Test (proptest) für
  Autosave/„ungespeichert“, ein Wächtertest gegen deutsche Texte außerhalb `text.rs` (egui_kittest über den
  Barrierefreiheitsbaum), Angriffs-/Schadenstests des Dateiformats, Müll-Parameter-Fuzzer für
  jeden Befehl, Snapshot-Tests der Abfragen.
- **App ausprobieren (Mac):** `cd ~/projekte/plnwrk && cargo xtask app`, dann
  `open target/plnwrk.app` (oder im Finder doppelklicken; nach `/Programme` ziehen geht auch).
- **CLI ausprobieren:**
  ```sh
  cd ~/projekte/plnwrk && cargo build -q -p plnwrk-cli
  ./target/debug/plnwrk-cli new ~/Desktop/abteilung.plnwrk --sample
  ./target/debug/plnwrk-cli export ~/Desktop/abteilung.plnwrk --format markdown
  ./target/debug/plnwrk-cli encrypt ~/Desktop/abteilung.plnwrk   # Passphrase ≥ 12 Zeichen
  ```
- **Browser-Vorschau** (Claude Desktop): Eintrag `plnwrk-web` in `~/.claude/launch.json`
  (trunk, Port 8087).

## 3. Nächste Schritte (in dieser Reihenfolge)

0. **Phase 5 fertigstellen:** Review-Befunde aus §3.1 beheben (mit Regressionstest und
   Mutationsprobe je Befund), `cargo xtask ci`, Nachreview, dann Merge nach Tobis „ja, merge“
   (Windows per Label `merge`).
1. **Realitätstest** mit einem erfundenen, aber typischen Vorgang der Abteilung (z. B. SKA von
   Eingang bis Abgabe) – möglichst durch eine zweite Person (offene Frage 4.10, überfällig).
2. **Tobi testet 3b-3** (Demo im Plan `docs/plans/2026-10-14-phase-3b-3.md`): ⌘K, Englisch,
   Dunkel. Palette, Englisch und Dunkelmodus (Lesbarkeit, Überfällig-Rot) hat der Agent in der
   echten App geprüft; offen ist Tobis Urteil, ob die Palette im Alltag trägt.
3. **Autosave in der echten App**: am 2026-10-14 belegt, dass eine Kopie im Hintergrund entstand
   und nach dem Beenden der Instanz (SIGTERM) auf der Startseite angeboten wurde. Noch nicht
   gezielt geprüft: minimiert + „Sofort beenden“ in der Aktivitätsanzeige.
4. **Tobi testet Phase 4** (Demo im Plan `docs/plans/2026-10-14-phase-4.md`), besonders das
   Hochladen über den echten Dateidialog (nur über Tests belegt).
5. **Tobi testet Phase 5** (Demo im Plan `docs/plans/2026-10-14-phase-5.md`): Zeitplan,
   Balken ziehen, „Wartet auf“.
6. **Veröffentlichen?** Der Workflow `pages` (von Hand) braucht eine eigene (Sub-)Domain.
7. Danach **4.6 / Phase 6 (KI, MCP)** – von Tobi für „heute Nacht“ angekündigt.
8. Optional: `cargo xtask mutants` (5.1).

### 3.1 Offene Review-Befunde Phase 5 (Review 2026-10-14, kein Blocker)

Bereits erledigt (Commit `0fda783`): Feiertagsliste je Jahr, Arbeitstage zählen ohne
Tagesschleife (`Calendar::count`), Helfer `Calendar::after_end` / `from_start` – **noch nicht**
in `plan.rs` verwendet.

1. **Getippter Termin + ⌘S/Schließen/anderer Aufgabe schiebt keine Nachfolger:** `flush_draft`
   (`crates/ui/src/lib.rs`) schickt Beginn/Frist per `task.update`. → Datumsteil aus
   `TaskDraft::changes()` herauslösen und über `PlnwrkApp::reschedule` schicken.
2. **Gantt leer bei alten Aufgaben:** `views/schedule.rs` beginnt bei der frühesten Aufgabe,
   kappt bei 800 Tagen; heute/offene Aufgaben fallen heraus. → Fenster um heute und die offenen
   Aufgaben legen.
3. **Frist auf Wochenende/Feiertag um einen Arbeitstag versetzt:** Ende-bezogene Bedingungen
   (`plan.rs` `requires`, Rückwärtsrechnung, `latest_start`) mit `after_end`, Anfang-bezogene mit
   `from_start` rechnen; Eigenschaftstest auch mit Wochenend-Daten.
4. **Langsam bei einem fernen Datum:** Zählen ist behoben; zusätzlich `Plan`/`Analysis` je
   Projekt und Änderungsstand (Länge des Protokolls) im egui-Speicher zwischenspeichern statt
   jedes Bild neu.
5. **Meilensteine immer kritisch (rot):** Puffer eines Meilensteins = Arbeitstage zwischen dem,
   was seine Vorgänger frühestens erlauben, und seiner Frist; ohne Vorgänger nicht kritisch.
6. **Erledigte Aufgaben werden mitgeschoben:** → fest wie Meilensteine, nicht kritisch, keine
   Warnungen über sie (**entschieden**, 4.12). Ihre Termine begrenzen Nachfolger weiterhin.
7. **„Wartet auf“ bietet Kreise an, Meldung passt nicht:** Aufgaben, die (direkt oder nicht)
   schon auf diese warten, nicht anbieten; `text::rule(Rule::Cycle)` allgemein formulieren.

Kleinigkeiten (nach Aufwand): Durchgang durch Aufgaben ohne Datum zählt im Rückwärtslauf einen
Tag (Dauer 0 statt 1); doppelte Meilenstein-Warnungen zusammenfassen (spätestes „frühestens“);
Meilenstein mit Beginn nach dem Ziehen (Beginn verwerfen, wenn er nach der neuen Frist läge);
Titel-Spalte klickbar machen, verdeckte Balken nicht ziehbar; Zieh-Vorschau mit `Plan::shifted`
statt Kalendertagen; Meldung bei mehr als 1000 Schüben.

## 4. Offene Entscheidungen des Nutzers

| # | Frage | Vorschlag | Spätestens |
| --- | --- | --- | --- |
| 4.1 | ~~Browser und VS-NfD~~ **entschieden:** harte Sperre im Browser (Phase 4) | – | erledigt |
| 4.2 | **Herabstufung**: Wer darf eine Datei herabstufen, braucht es vier Augen? Herabstufung erzeugt eine neue Datei ohne Geschichte. | Eigener, protokollierter Vorgang mit Begründung; Vier-Augen-Prinzip | vor Phase 7 |
| 4.3 | **Passphrase vergessen = Daten verloren.** Hinterlegung oder zweiter Schlüssel (age kann mehrere Empfänger)? Wer entscheidet in der Organisation? | Zweiter Empfänger (Behördenschlüssel) als Option | vor echtem VS-Einsatz |
| 4.4 | ~~Tempo vs. VS-Niveau in Phase 3~~ **entschieden:** früh zeigbar, VS später | – | erledigt |
| 4.5 | **Lizenz**: Apache-2.0 (wie RustGIS) oder EUPL-1.2 (öffentliche Hand, openCode)? | EUPL-1.2 prüfen | vor Phase 8 |
| 4.6 | **KI früher?** MCP-Server (Phase 6) könnte schon auf Dateien laufen und plnwrk vor weiterer UI-Arbeit aus Claude bedienbar machen. | Option, ändert Phasenfolge | jederzeit |
| 4.7 | **Identität im Protokoll**: „Ich arbeite als“ ist frei wählbar, nicht nachgewiesen – der Verlauf wirkt wie ein Nachweis, ist aber keiner. Signatur (Dienstausweis, Schlüsseldatei)? Bis dahin in der App kennzeichnen? | Klären vor Phase 7; Hinweis in der App erwägen | vor Phase 7 |
| 4.8 | **VS-NfD ohne Passphrase**: Beim Anlegen eines VS-NfD-Projekts sofort zum Schützen zwingen? Heute lässt sich unverschlüsselt weiterarbeiten; Speichern und Autosave scheitern dann (Statuszeile rot). | Beim Einstufen direkt den Schutz-Dialog öffnen | vor Phase 7 |
| 4.9 | **Wiederherstellungskopien von VS-Dateien** liegen (verschlüsselt) in einem Systemordner, den IT-Sicherheit kaum im Blick hat. Autosave bei VS aus, oder Kopie neben der Datei? | Für VS: Kopie im Ordner der Datei | vor echtem VS-Einsatz |
| 4.10 | **Wer testet außer Tobi**, mit welchem realistischen (erfundenen) Vorgang? | Eine Person aus der Abteilung, SKA-Ablauf | vor 3b-3 |
| 4.12 | ~~Erledigte Aufgaben im Zeitplan~~ **entschieden (Tobi, 2026-10-14):** fest wie Meilensteine, nicht kritisch, keine Warnungen über sie | – | umsetzen mit §3.1 Nr. 6 |
| 4.11 | **Fristen ohne Zeitzone**: „14:00“ gilt immer als Ortszeit des Geräts. Für Hamburg richtig; relevant bei Bund/EU-Bezug oder Arbeiten im Ausland? | So lassen, bis ein Fall auftritt | bei Bedarf |

## 5. Offene technische Punkte

### 5.1 Vorgeschlagen, noch nicht beauftragt
- **`.gitattributes`** mit `* text=auto eol=lf`: Windows checkt sonst CRLF aus (der Text-Wächter
  schlug deshalb nur in der Windows-CI an; seit 3b-3 normalisiert er selbst).
- **`cargo xtask push`**: pusht nur nach grüner Schranke im selben Lauf.
- **Sprache aus dem System** übernehmen (heute Standard Deutsch, Umschalten im Menü).
- **`cargo xtask mutants`** (cargo-mutants, nicht blockierend) für `store`, `engine`, `ui`.
- **Prüfschritt „ignorierte Testdateien“** in `cargo xtask ci` (die v1-Fixtures fehlten einmal
  unbemerkt im Repo, weil `.gitignore` sie ausschloss).
- **Aufräumen alter Wiederherstellungskopien** auf der Startseite (z. B. älter als 30 Tage).
- **Echte Vorgangsvorlagen** der Abteilung (SKA, Beschaffung, Übung) als Vorlagendatei.
- **Wiederkehrende Fristen** (z. B. Lagemeldung täglich 10:00) – Kandidat für Phase 5.
- **Eingangskanäle** (Mail, Drucksache/parldok-Import) als spätere Erweiterung.

### 5.2 Bekannte, bewusst akzeptierte Grenzen (Details PLAN.md §9)
- **Datei:** Rollback einer verschlüsselten Datei auf eine ältere Fassung mit derselben
  Passphrase wird nicht erkannt; eine unverschlüsselte Datei kann bei vollständiger Neuberechnung
  von Kette und Zustand unbemerkt verändert werden (beides: Signatur in Phase 7).
  „Reproduzierbares Speichern“ nur innerhalb eines Builds. Sperrdateien `.<name>.lock` bleiben
  liegen; Dateien in schreibgeschützten Ordnern lassen sich nicht öffnen. 0600-Rechte nur unter
  Unix. `SaveOptions.work_factor` erlaubt niedrigere scrypt-Faktoren (für Tests).
- **App:** Speichern verschlüsselter Dateien blockiert ≈ 1 s. Ohne macOS-Standardmenü fehlen
  Cmd+H/Cmd+M; Cmd+Q ruht, solange ein Dialog offen ist. Doppelklick auf `.plnwrk` im Finder
  öffnet noch nichts (Apple Event). Uhrzeiten werden intern
  als `HH:MM:SS` gespeichert (Eingabe `HH:MM`). Frist „00:00“ liest sich am Vorabend als
  „morgen fällig“.
- **Zeitplan:** Nachfolger rutschen beim Ändern von Terminen im Detailbereich und beim Ziehen
  im Zeitplan mit – nicht, wenn ein getippter, noch nicht übernommener Termin erst beim
  Speichern/Schließen übernommen wird, und nicht bei `task.update` über CLI/MCP (dort
  `task.reschedule` mit selbst berechneten Schüben). Eine neue Abhängigkeit verschiebt nichts,
  sondern wird gewarnt, falls sie nicht passt. Kalender fest Hamburg (Mo–Fr ohne Feiertage);
  Oberaufgaben fassen ihre Unteraufgaben nicht zusammen; in der Oberfläche nur „Ende →
  Anfang“ anlegbar (die anderen Arten über CLI). Der Zeitplan zeigt höchstens 800 Tage.
- **Sprache:** Kopien-Namen („– Wiederherstellung“, Ordner „Notkopien“) und die
  VS-NfD-Kennzeichnung bleiben deutsch; Daten (Titel, Art „Projekt“/„Vorgang“) bleiben, wie
  getippt. Technische Gründe aus Speicher/Engine (`reason`) erscheinen englisch hinter der
  übersetzten Meldung. Meldungen in der Statuszeile bleiben nach einem Sprachwechsel in der alten
  Sprache, bis die nächste kommt.
- **Art-Vorschläge** sind sprachabhängig („Vorgang“/„Case“); weil die Art Freitext ist,
  können Dateien beide Wörter enthalten.
- **Getippter, nicht übernommener Text** (Feld hat noch den Cursor) steht nicht in der
  Minutenkopie, nur in der Notsicherung beim Beenden; bei einem harten Absturz ist er weg.
- **Browser:** Zwischenstand liegt im `localStorage` (Klartext, nur nicht eingestufte,
  unverschlüsselte Arbeit); wer den Browser-Speicher lesen kann, sieht ihn – auch jede andere
  Seite unter derselben Adresse (darum vor dem Veröffentlichen eine eigene (Sub-)Domain statt
  `<owner>.github.io`). Verschlüsselte Arbeit bekommt keinen Zwischenstand. Speichern =
  Herunterladen; ob der Browser die Datei wirklich ablegt, erfährt die Seite nicht, darum
  bleibt die Kopie danach 7 Tage (markiert „heruntergeladen“; „Alle Zwischenstände löschen“
  auf der Startseite für gemeinsam genutzte Rechner). Offene Tabs halten eine Browser-Sperre
  (Web Locks: Chrome, Firefox, Safari ≥ 15.4) auf ihre Kennung; ohne Web Locks gelten andere
  Tabs als geschlossen (dann können zwei Tabs dieselbe Kopie übernehmen). Öffnen per Tastatur (⌘O, Palette) könnte Safari als
  „nicht vom Menschen ausgelöst“ ignorieren – Knopf „Datei öffnen…“ geht immer. Der
  Dateidialog des Browsers lässt sich nicht automatisiert prüfen.
  eframe registriert sein eigenes Speichern beim Verlassen unter einem falschen Ereignisnamen
  (`onbeforeunload`); plnwrk verlässt sich darum nicht darauf.
- **Autosave:** macOS „App Nap“ kann den Takt eines verdeckten Programms verzögern; nach
  Rückgängig bis zum gespeicherten Stand und Wiederholen entsteht die Kopie erst 60 s später neu;
  Entschützen einer ungespeicherten VS-NfD-Datei fragt erst nach dem Ort und lehnt dann ab.
  Passphrasen können im Undo-Puffer von Textfeldern (nur RAM) verbleiben.
- **Werkzeug:** Abdeckungszahl enthält In-Datei-Testmodule.

### 5.3 Für spätere Phasen vorgemerkt
- **Phase 7 (VS):** Herabstufung (4.2), Signatur (4.7), Export-Verweigerung bei Herabstufung,
  Audit-Bericht, Entscheidungen 4.8/4.9.

## 6. Arbeitsweise, die sich bewährt hat

- Phase → Phasenplan (`docs/plans/`) → TDD → `cargo xtask ci` grün → PR → **unabhängiger
  Review-Agent (Opus)** → Befunde einarbeiten → **Nachreview** → CI grün (Windows per Label
  `merge`) → Squash-Merge.
- Reviews fanden in **jeder** Phase echte Fehler. In der Oberfläche fast immer dieselbe Klasse:
  getippte, noch nicht übernommene Eingaben gehen bei Speichern/Schließen verloren → zentrale
  Stelle `has_unsaved_changes` + `flush_draft` (Regel in `CLAUDE.md`). Bei Formatänderungen:
  alte Dateien, Undo über Umbauten, stabile IDs.
- Jede Aussage „Test fängt X“ per Mutationsprobe belegen (Skript, das eine Zeile ändert, Tests
  laufen lässt, zurücksetzt); Skript-Ersetzungen mit `assert old in s` absichern.
- Oberflächentests mit egui_kittest: Felder per `labelled_by` mit Beschriftung verknüpfen,
  Symbol-Knöpfe per `views::icon_button` benennen; Text von Labels steht im Knoten unter `value`.
- Die echte App lässt sich über die Computer-Use-Werkzeuge prüfen (Bundle-ID
  `app.plnwrk.desktop`, `cargo xtask app`); Bildschirmaufnahmen fallen gelegentlich aus.
- Windows-only-Probleme zeigt nur die CI.

## 7. Wichtige Dateien

| Datei | Inhalt |
| --- | --- |
| `docs/PLAN.md` | verbindlicher Plan, Regeln, Architektur, Entscheidungsprotokoll §9 |
| `docs/STATUS.md` | dieses Dokument |
| `docs/plans/2026-10-14-phase-*.md` | Aufgabenpläne der Phasen (0, 1, 2, 3, 3b-1, 3b-2, 3b-3) mit Demo-Schritten |
| `crates/ui/src/text.rs` | alle Oberflächentexte, Deutsch und Englisch |
| `crates/ui/src/palette.rs` | Befehlspalette (Einträge, Suche, Ausnahmeliste) |
| `crates/ui/src/model_tests.rs` | Zustandsautomat-Test für Autosave und „ungespeichert“ |
| `CLAUDE.md` | Agenten-Einstieg, Befehle, Stolperfallen, Regeln |
| `xtask/src/{ci,layers,purity,app}.rs` | die Schranke und der App-Bau |
| `crates/store/tests/store.rs`, `crates/store/tests/fixtures/` | Angriffs-/Schadenstests, echte v1-Dateien |
| `crates/ui/src/tests.rs` | Oberflächentests (klicken und tippen wie eine Person) |
| `crates/ui/src/autosave.rs` | Autosave und Wiederherstellung |

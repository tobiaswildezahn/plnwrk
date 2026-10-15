# plnwrk – Stand und offene Punkte

> Übergabedokument für die nächste Session (Mensch oder Agent). Stand: **2026-10-14 abends:
> Phase 6a (MCP-Server) gemergt, Repository öffentlich** (2026-10-15); 6b (Assistent in
> der App) ist ausgesetzt. Verbindlich bleibt [PLAN.md](PLAN.md) (Entscheidungsprotokoll §9);
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
| 5 Zeitplanung (jeder Tag zählt, Nachfolger schieben, kritischer Pfad, Gantt) | **fertig**, Review + Nachreview + Kurzprüfung | [#9](https://github.com/tobiaswildezahn/plnwrk/pull/9) |
| 6 KI (MCP, Assistent) | 6a MCP-Server **fertig**, Review + Nachreview; 6b ausgesetzt (Tobi) | [#10](https://github.com/tobiaswildezahn/plnwrk/pull/10) |
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

0. **Tobi probiert den MCP-Server** an `~/Desktop/test.plnwrk` / `test2.plnwrk` in einer neuen
   Claude-Code-Sitzung (eingerichtet, `docs/mcp.md`). Wichtigste Frage: Kommt Claude ohne
   Umwege mit den Werkzeugen zurecht? Nach jedem Update: `cargo install --path apps/plnwrk-cli
   --locked`. Kleinigkeit notiert: „Datei existiert schon“ als eigene Fehlerart statt `io`.
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
7. 6b (Assistent in der App, lokales Modell) – ausgesetzt, „etwas für später“ (Tobi, 2026-10-14).
8. Optional: `cargo xtask mutants` (5.1).

### 3.1 Review-Befunde Phase 5 – behoben (2026-10-14)

Alle 7 Befunde des Reviews und alle Kleinigkeiten sind behoben, jeweils mit Regressionstest und
Mutationsprobe (`88092e1`, `944cae9`). Das Nachreview fand keinen Blocker, aber 6 weitere Punkte;
auch die sind behoben (`e824e71`):
- Eine früh erledigte Aufgabe mit spätem Termin verdeckte den kritischen Pfad.
- Bei einer erledigten Aufgabe ohne Datum in einer Kette stimmte der Puffer nicht.
- Eine Aufgabe ganz auf Wochenende/Feiertag wurde uneinheitlich verankert.
- Ferne Termine waren langsam (Arbeitstage werden jetzt halbierend statt tageweise gesucht).
- Erledigte Projekte zeigten einen leeren Gantt.
- Beim Speichern mit getippter Frist wurde ein anderswo geänderter Beginn zurückgeschrieben.

Eine Kurzprüfung dieses Commits fand eine dadurch entstandene Verschlechterung: Eine Aufgabe
nur mit Beginn an einem Samstag galt als Freitag (falsche Warnung). Behoben in `f03bfc4`, ebenso das
Doppelzählen der letzten Kalendertage (Jahr 9999) und eine verlorene „mitverschoben“-Meldung.

Zusätzlich belegt ein Eigenschaftstest über zufällige Pläne die Puffer: Rutscht eine Aufgabe um
ihren Puffer, ändert sich nichts; rutscht sie einen Tag mehr, verschiebt sich das Ende oder ein
Meilenstein wird unhaltbar.

## 4. Offene Entscheidungen des Nutzers

| # | Frage | Vorschlag | Spätestens |
| --- | --- | --- | --- |
| 4.1 | ~~Browser und VS-NfD~~ **entschieden:** harte Sperre im Browser (Phase 4) | – | erledigt |
| 4.2 | **Herabstufung**: Wer darf eine Datei herabstufen, braucht es vier Augen? Herabstufung erzeugt eine neue Datei ohne Geschichte. | Eigener, protokollierter Vorgang mit Begründung; Vier-Augen-Prinzip | vor Phase 7 |
| 4.3 | **Passphrase vergessen = Daten verloren.** Hinterlegung oder zweiter Schlüssel (age kann mehrere Empfänger)? Wer entscheidet in der Organisation? | Zweiter Empfänger (Behördenschlüssel) als Option | vor echtem VS-Einsatz |
| 4.4 | ~~Tempo vs. VS-Niveau in Phase 3~~ **entschieden:** früh zeigbar, VS später | – | erledigt |
| 4.5 | ~~Lizenz~~ **faktisch entschieden (2026-10-15):** mit dem Veröffentlichen gilt Apache-2.0 (LICENSE); EUPL bliebe für spätere Versionen möglich | – | erledigt |
| 4.6 | ~~KI früher?~~ **entschieden (Tobi, 2026-10-14):** MCP-Server jetzt (6a), Assistent in der App ausgesetzt (6b) | – | umgesetzt mit 6a |
| 4.7 | **Identität im Protokoll**: „Ich arbeite als“ ist frei wählbar, nicht nachgewiesen – der Verlauf wirkt wie ein Nachweis, ist aber keiner. Signatur (Dienstausweis, Schlüsseldatei)? Bis dahin in der App kennzeichnen? | Klären vor Phase 7; Hinweis in der App erwägen | vor Phase 7 |
| 4.8 | **VS-NfD ohne Passphrase**: Beim Anlegen eines VS-NfD-Projekts sofort zum Schützen zwingen? Heute lässt sich unverschlüsselt weiterarbeiten; Speichern und Autosave scheitern dann (Statuszeile rot). | Beim Einstufen direkt den Schutz-Dialog öffnen | vor Phase 7 |
| 4.9 | **Wiederherstellungskopien von VS-Dateien** liegen (verschlüsselt) in einem Systemordner, den IT-Sicherheit kaum im Blick hat. Autosave bei VS aus, oder Kopie neben der Datei? | Für VS: Kopie im Ordner der Datei | vor echtem VS-Einsatz |
| 4.10 | **Wer testet außer Tobi**, mit welchem realistischen (erfundenen) Vorgang? | Eine Person aus der Abteilung, SKA-Ablauf | vor 3b-3 |
| 4.12 | ~~Erledigte Aufgaben im Zeitplan~~ **entschieden (Tobi, 2026-10-14):** fest wie Meilensteine, nicht kritisch, keine Warnungen über sie | – | umgesetzt |
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
- **Zeitplan:** Gerechnet wird mit **jedem Tag** (Entscheidung 2026-10-15: Gefahrenabwehr
  arbeitet auch am Wochenende und an Feiertagen; eine Woche = 7 Tage). Der Gantt hinterlegt
  Wochenenden und Hamburger Feiertage nur zur Orientierung; der Hamburg-Kalender (Mo–Fr) steht
  in `plnwrk-schedule` für eine spätere Wahl je Projekt bereit. Nachfolger rutschen bei jeder
  Terminänderung in der Oberfläche und über MCP (`task.update`, `move_task`) mit – nicht bei
  `task.update` über die CLI (dort `task.reschedule` mit selbst berechneten Schüben). Erledigte
  Aufgaben stehen fest (4.12). Mehr als 1000 Schübe in einem Schritt werden mit Meldung
  abgelehnt. Eine neue Abhängigkeit verschiebt nichts, sondern wird gewarnt, falls sie nicht
  passt.
  Oberaufgaben fassen ihre Unteraufgaben nicht zusammen; in der Oberfläche nur „Ende →
  Anfang“ anlegbar (die anderen Arten über CLI). Der Zeitplan zeigt höchstens 800 Tage: alles,
  wenn es passt, sonst ab der frühesten offenen Aufgabe (heute bleibt sichtbar).
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

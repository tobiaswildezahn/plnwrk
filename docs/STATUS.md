# plnwrk – Stand und offene Punkte

> Übergabedokument für die nächste Session (Mensch oder Agent). Stand: **2026-10-17 abends:
> plnwrk 0.2.0 veröffentlicht** (Release + Browser unter plnwrk.wildezahn.de); **Phase 9
> (Gestaltung) gebaut:** 9a–9g gemergt (#16–#22); offen ist der Wiederholungstest.
> 6b (Assistent in der App) ist ausgesetzt. Verbindlich bleibt [PLAN.md](PLAN.md) (Entscheidungsprotokoll §9);
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
| 6 KI (MCP, Assistent) | 6a MCP-Server **fertig**, Review + Nachreview; 6a+ (jeder Tag zählt, `move_task`, sprechende Antworten) **fertig**; 6b ausgesetzt (Tobi) | [#10](https://github.com/tobiaswildezahn/plnwrk/pull/10), [#11](https://github.com/tobiaswildezahn/plnwrk/pull/11) |
| 7 VS-Härtung | offen | – |
| 8 Veröffentlichung | **0.1.0 veröffentlicht** (2026-10-17): [Release](https://github.com/tobiaswildezahn/plnwrk/releases/tag/v0.1.0); **0.2.0** (2026-10-17, Phase 9): [Release](https://github.com/tobiaswildezahn/plnwrk/releases/tag/v0.2.0); Browser unter [plnwrk.wildezahn.de](https://plnwrk.wildezahn.de) | [#14](https://github.com/tobiaswildezahn/plnwrk/pull/14) |
| 9 Gestaltung und Bedienbarkeit | 9a Bedienfehler + Gestaltungssystem, 9b Ribbon, 9c Zeitplan, 9d Meilensteine (+ Bestätigung jeder Meilenstein-Änderung), 9e Board, 9f Dialoge/Start/Übersicht/Liste/Details/Leerzustände, 9g Meldungen/Klickpfade/Überholte Meilensteine **fertig**, je Review + Nachreview; offen: Wiederholungstest | [#16](https://github.com/tobiaswildezahn/plnwrk/pull/16)–[#22](https://github.com/tobiaswildezahn/plnwrk/pull/22) |

Alle Phasen ab 3a: Review + Nachreview durch unabhängigen Agenten, Befunde eingearbeitet,
Mutationsproben, CI grün auf Linux, macOS, Windows und Web.

## 2. Technischer Ist-Zustand

- **Repo:** `~/projekte/plnwrk/`, GitHub öffentlich (seit 2026-10-15) `tobiaswildezahn/plnwrk`. `main` enthält
  Phase 0–9 (Stand `70557f9`, Version 0.2.0). Neue Phase immer auf eigenem Branch (`phase-…`).
- **Veröffentlichen** (nur auf Tobis „veröffentlichen“), drei Schritte: (1) Version in `Cargo.toml`
  per PR, gemergt, CI auf `main` grün; (2) Tag `v<version>` auf diesen Commit pushen – der
  Release-Ablauf baut und veröffentlicht die Pakete; (3) **Pages-Ablauf von Hand starten**
  (`gh workflow run pages.yml --ref main`) – die Browser-Version folgt `main` **nicht** von
  selbst. Danach README „Stand“ per Doku-PR.
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
  Zufalls-IDs), `ui` (egui-Oberfläche inkl. Autosave, Gestaltungssystem `design/`, Ribbon),
  `schedule` (Plan, Analyse, Schub, „Meilenstein überholt“), `mcp` (MCP-Server), `assistant`
  (Gerüst, 6b ausgesetzt); Apps `plnwrk` (Desktop, Systemdialoge, ohne macOS-Standardmenü),
  `plnwrk-web` (dieselbe Oberfläche im Browser, mit Hochladen/Herunterladen), `plnwrk-cli`.
- **Tests:** 654 im ganzen Arbeitsbereich, davon 315 Oberflächentests ohne Fenster (egui_kittest
  über den Barrierefreiheitsbaum, seit 9f auch über die gezeichneten Formen: `painted_texts`,
  `painted_fills`); jede Prüfung mit Gegenprobe (CLAUDE.md). Dazu ein Zustandsautomat-Test
  (proptest) für Autosave/„ungespeichert“, Wächter gegen deutsche Texte außerhalb `text.rs`,
  gegen Farben/Schriftgrößen außerhalb `design/` und für jedes Eingabefeld (zählt als
  ungespeichert oder steht mit Grund auf einer Liste), WCAG-Kontrasttest, Angriffs-/Schadenstests des Dateiformats, Müll-Parameter-Fuzzer für
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

0. **Phase 9 – Gestaltung und Bedienbarkeit** (Plan `docs/plans/2026-10-17-phase-9-gestaltung.md`):
   **9a–9f gemergt** (#16–#21; 9f: Anlegen-Dialog Vorgang | Projekt mit einer Regel
   `Project::is_case`, Startseite, Übersicht „Heute und überfällig“ mit Ampeln, Liste mit Spalten,
   Detailbereich in Gruppen, Leerzustände, Meldungsleiste, angeheftete Datumsleiste).
   **9g gemergt** (#22, Review + Nachreview eingearbeitet), **0.2.0 veröffentlicht**:
   (1) Meldung mit Ursprung (4.16), (2) Klickpfade, jeder auch
   nur mit Tastatur (4.17), (3) Übersicht-Block „Überholte Meilensteine“, (4) Durchgang von Hand,
   (5) Version 0.2.0. **Offen:** Wiederholungstest mit dem Sachbearbeiter (dabei 4.14, 4.15) und
   Entscheidung 4.18.
1. ~~Realitätstest~~ erledigt (2026-10-17), Wiederholungstest nach Phase 9f/9g.
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
6. ~~README und Phase 8~~ erledigt (#13, #14): 0.1.0 veröffentlicht, Browser unter eigener
   Subdomain plnwrk.wildezahn.de.
7. 6b (Assistent in der App, lokales Modell) – ausgesetzt, „etwas für später“ (Tobi, 2026-10-14).
8. Optional: `cargo xtask mutants` (5.1).
9. ~~Board/Zoom/Export~~ gemergt (#12, 2026-10-15); Board und Zeitplan seither in 9c/9e neu
   gestaltet.
10. Kleinigkeiten (notiert 2026-10-15): „Datei existiert schon“ als eigene Fehlerart statt `io`;
   Git-Hook, der eine geänderte, nicht vorgemerkte `Cargo.lock` abweist (zweimal rote CI);
   Hinweis bei Fristen auf Wochenende/Feiertag (ohne die Rechnung zu ändern) – Tobi entscheidet,
   siehe 4.13.

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
| 4.10 | ~~Wer testet außer Tobi~~ **erledigt (2026-10-17):** Tobis Sachbearbeiter; Befunde → Phase 9 |
| 4.12 | ~~Erledigte Aufgaben im Zeitplan~~ **entschieden (Tobi, 2026-10-14):** fest wie Meilensteine, nicht kritisch, keine Warnungen über sie | – | umgesetzt |
| 4.13 | **Fristen auf Wochenende/Feiertag**: Seit „jeder Tag zählt“ setzen Schub und `move_task` Termine auch auf Samstag/Sonntag. Für Einsatzvorhaben richtig, für Verwaltungsfristen (SKA, Senatskanzlei) evtl. falsch. Nur Hinweis anzeigen, Kalender je Projekt wählbar machen (Hamburg-Kalender liegt bereit) oder so lassen? | Erst im Realitätstest beobachten | nach 4.10 |
| 4.14 | ~~Ziehen nach „Erledigt“ bei offenen Vorgängern~~ **entschieden (Tobi, 2026-10-17):** weicher Hinweis bleibt, im Wiederholungstest beobachten | – | Wiederholungstest |
| 4.15 | ~~Eingabefelder „flach“?~~ **entschieden (Tobi, 2026-10-17):** Tobi urteilt am echten Bildschirm im Wiederholungstest, danach ggf. Rahmen/Höhe nachschärfen | – | Wiederholungstest |
| 4.16 | ~~Meldungen an ihren Ursprung binden~~ **entschieden (Tobi, 2026-10-17):** ja, als erster Schritt von 9g, vor den Klickpfaden | – | 9g, Schritt 1 |
| 4.17 | ~~Klickpfade auch nur mit Tastatur~~ **entschieden (Tobi, 2026-10-17):** ja, eigene Tastatur-Variante je Pfad | – | 9g |
| 4.18 | **Enter und Escape im Detailbereich** (Befunde 9g: Klickpfade und Review F3): Enter übernimmt ein Feld, aber der Cursor verlässt es; wer mit der Tastatur weiterarbeitet, fängt oben im Fenster an (Tab übernimmt und springt weiter, das geht). **Escape übernimmt das Getippte**, statt es zu verwerfen – beim Umbenennen in der Liste verwirft Escape, in Office auch. Soll Enter den Cursor im Feld lassen und Escape verwerfen? Berührt jeden Datumsfluss samt Meilenstein-Rückfrage; zwei Nachstell-Tests des Prüfers liegen bereit. | Ja, beides zusammen als eigener Schritt | 9g oder danach |
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
- **Tastatur (Befund 9g):** Wer ein Datum im Detailbereich ändert und das Feld mit
  **Umschalt+Tab** verlässt, verliert den Fokus (vorwärts mit Tab geht es; ohne Änderung auch
  rückwärts). Vermutlich die Fokusführung von egui nach dem Übernehmen. Noch nicht behoben,
  der Klickpfad „Balken eine Woche später“ geht deshalb vorwärts.
- **Schmale Fenster (Durchgang 9g, 800 px):** Mit Seitenleiste und offenem Detailbereich
  bleiben der Liste etwa 200 px; egui schneidet sie dann links ab, statt sie zu verkleinern.
  egui merkt sich zudem die Breite des Detailbereichs, auch eine ungewollt gewachsene. Geprüft
  ist 1280×800 und breiter (Plan §9).
- **Seitenleiste:** Das Symbol vor „Vorlagen“ steht weiter vom Text ab als bei „Vorgänge“ und
  „Projekte“ (Kleinigkeit, Durchgang 9g).
- **Tastaturpfade** (gemessen 9g, `crates/ui/src/tests/paths.rs`): Vorgang 9 Tasten, Projekt 5,
  Aufgabe mit Frist 15, Heute 2, Balken eine Woche 24, Meilenstein 15, Karte erledigt 4,
  Abhängigkeit 24. Über die Befehlspalette geht alles; Felder im Detailbereich erreicht man nur
  mit Tab der Reihe nach (kein Sprung zwischen Bereichen wie F6 in Office).
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
  übersetzten Meldung. Meldungen (seit 9f in der Leiste unter dem Menüband) bleiben nach einem
  Sprachwechsel in der alten Sprache, bis die nächste kommt.
- **Art:** Der Anlegen-Dialog schreibt „Vorgang“/„Projekt“ in der Sprache der Oberfläche; die
  Grundwörter (Vorgang/Case, Projekt/Project) werden seit 9f in der jeweils gewählten Sprache
  angezeigt, eigene Arten („SKA“) wie getippt. Ob etwas ein Vorgang ist, entscheidet überall
  `Project::is_case` (Aktenzeichen oder eine Art außer „Projekt“).
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

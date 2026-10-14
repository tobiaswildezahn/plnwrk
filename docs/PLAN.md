# Stabsplan – Architektur und Implementierungsplan

> **Dies ist das verbindliche Steuerdokument für alle Agenten und Sessions.**
> Vor jeder Arbeit lesen. Abweichungen hier eintragen, nicht stillschweigend umsetzen.
> Status der Phasen wird in Abschnitt 9 gepflegt.

Arbeitstitel: **Stabsplan** (Umbenennung jederzeit trivial, nur Cargo-Namen und Ordner).

## 1. Ziel und Leitplanken

**Was:** Eine Projektmanagement-Software, die die Basics vollständig abbildet und auf jedem
Desktop (macOS, Windows, Linux) und im Browser läuft. Vorbild in Aufbau und Qualitätsanspruch
ist [RustGIS](https://rustgis.app).

**Warum (Alleinstellung):**

1. **Datenhoheit** – jedes Projekt ist eine Datei, die dem Nutzer gehört. Kein Konto, keine Cloud,
   keine Telemetrie.
2. **Offline** – alles funktioniert ohne Netz. Netzzugriffe sind die Ausnahme und immer explizit.
3. **Volle KI-Integration** – ein Agent kann alles, was ein Mensch kann: über dieselben Befehle,
   in der App (Assistent), per Kommandozeile und als MCP-Server.
4. **VS-fähig (Fernziel)** – geeignet für Verschlusssachen bis VS-NfD: Verschlüsselung,
   Revisionsspur, Einstufungskennzeichen, lokale KI, Netz-Sperre. Hinweis: Die Zulassung zur
   VS-Verarbeitung erteilt die Geheimschutzstelle für das Gesamtsystem; die Software schafft die
   technischen Voraussetzungen.

**Veröffentlichung:** Open Source, Lizenz Apache-2.0 (wie RustGIS; Alternative EUPL-1.2 für
Behördenkontext wird vor Veröffentlichung entschieden). Repo erst privat, dann öffentlich.

**Nicht-Ziele (vorerst):** Zeiterfassung/Abrechnung, Ressourcenkostenrechnung, Chat, Dateiablage,
Mehrbenutzer-Echtzeit. Letzteres ist durch die Architektur vorbereitet (Abschnitt 3.4), aber nicht
Teil der ersten Version.

## 2. Nicht verhandelbare Regeln

Diese Regeln sind von RustGIS übernommen und gelten für jeden Commit.

| Regel | Konkret |
| --- | --- |
| **Nur Rust** | Keine JavaScript-Logik, kein Python. Erlaubt: generiertes HTML/CSS-Gerüst für den Web-Start (`index.html`), Shell für CI. |
| **Schichtung** | Untere Crates dürfen nie von oberen abhängen. Wird automatisch geprüft (`cargo xtask ci`). |
| **Alles ist ein Befehl** | Jede Nutzeraktion ist ein `CommandSpec`. UI, CLI, MCP und Assistent rufen ausschließlich `Session::run`. |
| **Never crash** | Kein `unwrap`, `expect`, `panic!`, Indexzugriff ohne Prüfung in Produktionscode (Clippy `unwrap_used`, `expect_used`, `panic` = deny). Dateiinhalte und Befehlsparameter sind feindlich. |
| **Tests zuerst** | Neue Fachlogik entsteht per TDD. Jeder Befehl wird zusätzlich mit Junk-Parametern getestet. |
| **Eine Schranke** | `cargo xtask ci` = fmt-Check, clippy `-D warnings`, alle Tests, Schichtprüfung, `cargo deny` (Lizenzen/Advisories), WASM-Build. Rot = nicht fertig. |
| **Kein Netz ohne Erlaubnis** | Jede Funktion, die das Gerät verlässt, geht durch `core::policy::NetworkPolicy` und ist standardmäßig aus. |
| **Deterministischer Kern** | `core` und `engine` sind frei von I/O, Uhrzeit, Zufall. Diese kommen von außen herein (Clock-/Id-Trait). Dadurch testbar und WASM-fähig. |
| **Sprache** | Code, Bezeichner, Commit-Messages: Englisch. UI-Texte über i18n, Deutsch und Englisch von Anfang an. Doku: Deutsch (dieses Repo), Englisch vor Veröffentlichung. |

## 3. Architektur

### 3.1 Workspace-Schichten

```
stabsplan/
├── Cargo.toml                 # Workspace
├── CLAUDE.md                  # Agenten-Einstieg → verweist hierher
├── docs/PLAN.md               # dieses Dokument
├── xtask/                     # cargo xtask ci, layering check
├── crates/
│   ├── core/        L0  Domänenmodell, Ids, Zeit, Validierung, Policy
│   ├── engine/      L1  Session, Befehlsregister, Events, Undo/Redo, Abfragen
│   ├── store/       L2  Dateiformat, Verschlüsselung, Laden/Speichern, Migration
│   ├── schedule/    L2  Zeitplanung: Abhängigkeiten, kritischer Pfad, Kalender
│   ├── assistant/   L3  KI-Agent über Befehle; Provider: Ollama/lokal, Anthropic, OpenAI-kompatibel
│   ├── mcp/         L3  MCP-Server (stdio) auf Basis von engine
│   └── ui/          L4  egui-Oberfläche (Desktop und Web identisch)
└── apps/
    ├── stabsplan/        Desktop (eframe nativ)
    ├── stabsplan-web/    Browser (eframe → wasm32, trunk)
    └── stabsplan-cli/    Kommandozeile: run, query, export, mcp
```

Abhängigkeitsrichtung strikt nach unten. `ui` kennt `engine`, nie umgekehrt.
`assistant` und `mcp` kennen nur `engine`, nie `ui`.

### 3.2 Domänenmodell (`core`)

Die vollständigen Basics des Projektmanagements:

| Entität | Felder (Kern) |
| --- | --- |
| `Project` | id, name, description, start, end?, status (planned/active/on_hold/done/archived), classification |
| `Task` | id, project, parent? (Unteraufgaben beliebig tief), title, description (Markdown), status (todo/in_progress/blocked/review/done), priority (low/normal/high/critical), assignee?, due?, start?, estimate?, progress 0–100, tags, classification |
| `Milestone` | id, project, title, date, status (open/reached/missed) |
| `Dependency` | from_task, to_task, kind (finish_to_start, start_to_start, finish_to_finish), lag_days |
| `Person` | id, name, email?, role, initials |
| `Comment` | id, task, author, created, body |
| `Classification` | open / vs_nfd (erweiterbar), gilt pro Projekt und pro Aufgabe; Aufgabe erbt vom Projekt, darf nur höher sein |

Ids: ULID (zeitlich sortierbar, offline erzeugbar, später sync-fähig). Zeiten: `jiff` oder `time`,
immer UTC in Daten, Lokalzeit nur in der UI.

Validierung lebt in `core` (z. B. kein Zyklus in Abhängigkeiten, due ≥ start, Unteraufgabe nicht
eigener Vorfahr) und gibt typisierte Fehler zurück, nie Panics.

### 3.3 Befehle und Events (`engine`)

```rust
pub struct CommandSpec {
    pub id: &'static str,          // "task.create", "task.move", "milestone.reach"
    pub label: &'static str,       // i18n-Schlüssel
    pub params: ParamSchema,       // JSON-Schema-ähnlich; dient UI-Formularen, CLI und MCP
    pub enabled: fn(&Session) -> bool,
    pub run: fn(&mut Session, Params) -> Result<Outcome, CommandError>,
}
```

- **`Session::run(id, params)`** ist der einzige Weg, Zustand zu ändern. Jeder Lauf ist ein
  Undo-Schritt. Schlägt der Befehl fehl, bleibt der Zustand unverändert (Transaktion).
- Jeder erfolgreiche Befehl erzeugt **Events** (`TaskCreated`, `TaskMoved`, …), die an ein
  Append-Only-Log gehen. Der Projektzustand ist eine Faltung der Events. Das ist zugleich die
  **Revisionsspur** (VS) und die **Grundlage für späteren Team-Abgleich** (3.4).
- **Abfragen** (`Query`) sind lesend und werden nicht geloggt: Aufgabenlisten mit Filter/Sortierung,
  Board-Spalten, Zeitplan, Überfälliges, Fortschritt je Projekt.
- Das Befehlsregister ist zur Laufzeit durchsuchbar (`search_commands`), damit UI-Palette, CLI
  und MCP dieselbe Quelle nutzen.

### 3.4 Speicherung (`store`) und Offline-/Team-Vorbereitung

- **Dateiformat `.stabsplan`:** ZIP-Container mit `manifest.json` (Formatversion, Einstufung,
  Erstellt-mit), `snapshot.json` (aktueller Zustand) und `events.jsonl` (Log). Lesbar, diffbar,
  langfristig archivierbar.
- **Verschlüsselung (optional, Pflicht bei Einstufung ≥ VS-NfD):** gesamter Container mit
  `age`-kompatiblem Verfahren (X25519/ChaCha20-Poly1305, Passphrase-Modus über scrypt).
  Im Klartext bleibt nur eine Kopfzeile mit Formatversion und Einstufung.
- **Desktop:** Datei öffnen/speichern über `rfd`, Autosave in temporäre Datei, atomares
  Umbenennen. **Web:** Datei per Upload/Download, Zwischenstand in IndexedDB (über `web-sys`,
  bleibt Rust).
- **Team später:** Weil alles Events mit ULIDs sind, kann ein künftiger `server`-Crate Event-Logs
  zusammenführen (Last-Writer-Wins je Feld, Konflikte als Kommentar sichtbar). Nichts im Kern
  muss dafür umgebaut werden; deshalb heute **kein** Server.

### 3.5 Zeitplanung (`schedule`)

Abhängigkeitsgraph, topologische Sortierung, früheste/späteste Termine, kritischer Pfad,
Arbeitskalender (Wochenenden, Feiertage als Liste). Liefert Daten für Gantt-Ansicht und
Warnungen („Meilenstein nicht haltbar“). Reine Funktionen, vollständig unit-getestet.

### 3.6 Oberfläche (`ui`, egui)

Ein Code für Desktop und Browser (wie RustGIS: `egui`/`eframe`, Web über `wasm32-unknown-unknown`
mit `trunk`). Ansichten:

1. **Projektübersicht** – Karten je Projekt mit Fortschritt, Überfälliges, nächster Meilenstein
2. **Liste** – Tabelle mit Filter, Sortierung, Gruppierung, Inline-Bearbeitung
3. **Board** – Kanban nach Status, Drag & Drop
4. **Zeitleiste** – Gantt mit Abhängigkeiten, Meilensteinen, kritischem Pfad
5. **Detail** – Seitenpanel: alle Felder, Unteraufgaben, Kommentare, Verlauf (aus Events)
6. **Befehlspalette** (`Ctrl/Cmd+K`) – jeder Befehl per Tastatur
7. **Assistent** – Chat-Panel, das über Befehle handelt und jede Aktion sichtbar protokolliert

Barrierefreiheit über `accesskit` (bei egui enthalten). Theme hell/dunkel. i18n de/en.

### 3.7 KI (`assistant`, `mcp`)

- **Assistent in der App:** Tool-Loop wie bei RustGIS: Provider-Format (Anthropic Messages,
  OpenAI-kompatibel, Ollama) als reine Request/Response-Daten, der Transport (Thread nativ, `fetch`
  im Web) liegt in den Apps. Werkzeuge des Agenten = `search_commands`, `run_command`, `query`.
  Jede KI-Aktion ist ein Undo-Schritt und im Verlauf als „durch Assistent“ markiert.
- **Lokal zuerst:** Ollama/OpenAI-kompatibel auf `localhost` ist Standard. Externe Provider
  erfordern explizite Freigabe über `NetworkPolicy`; bei Einstufung ≥ VS-NfD ist extern gesperrt.
- **MCP-Server:** `stabsplan-cli mcp --project p.stabsplan` über stdio, Crate `rmcp`. Tools:
  `list_projects`, `query`, `search_commands`, `run_command`, `read_task`, `search_docs`,
  `read_doc`. Schreiben nur mit `--yes`. Damit kann Claude Code Projekte direkt bearbeiten.

### 3.8 VS-Härtung (Querschnitt)

| Anforderung | Umsetzung |
| --- | --- |
| Vertraulichkeit | Container-Verschlüsselung (3.4), keine Klartext-Temporärdateien, Zwischenablage-Hinweis |
| Integrität | Event-Log mit Hash-Kette (jedes Event enthält Hash des Vorgängers) |
| Nachvollziehbarkeit | Verlauf je Aufgabe aus Events: wer, wann, was, Assistent oder Mensch |
| Kennzeichnung | Einstufung sichtbar in Titelzeile, Druck/Export mit Kennzeichnung, Export verweigert Herabstufung |
| Abfluss-Kontrolle | `NetworkPolicy` zentral; bei VS-NfD hart „offline only“, Web-Build zeigt Warnung, dass Browserumgebung nicht VS-geeignet ist |
| Keine Telemetrie | gibt es nicht, auch keine Update-Prüfung ohne Klick |

## 4. Technologie-Entscheidungen

| Bereich | Wahl | Begründung |
| --- | --- | --- |
| Toolchain | Rust stable via `rustup`, Edition 2024 | Standard |
| UI | `egui` + `eframe` (Version wie RustGIS, 0.36.x) | Eine Codebasis Desktop + Web, bewährt in RustGIS |
| Web-Build | `trunk`, Ziel `wasm32-unknown-unknown` | Standardweg für eframe |
| Serialisierung | `serde`, `serde_json` | Universell, lesbares Dateiformat |
| Ids | `ulid` | Offline, sortierbar, sync-fähig |
| Zeit | `jiff` | Zeitzonen korrekt, moderne API |
| Container | `zip` | Wie RustGIS-Plugins |
| Krypto | `age` (Crate) | Geprüftes Format, Passphrase + Schlüssel |
| Hash | `blake3` | Schnell, für Event-Kette |
| Dateidialog | `rfd` | Nativ + Web |
| MCP | `rmcp` (offizielles Rust-SDK) | Standardkonform |
| HTTP (nativ) | `ureq` + `rustls` | Kein OpenSSL, wie RustGIS |
| CLI | `clap` | Standard |
| Tests | eingebaut + `proptest` (Junk-Parameter) + `insta` (Snapshots für Abfragen) | |
| Packaging | `cargo-dist` (dmg/msi/AppImage), GitHub Pages für Web | Reproduzierbare Releases |
| Doku | `mdbook` unter `docs/book/` | Wie RustGIS: Doku ist auch MCP-Ressource |

## 5. Entwicklungsumgebung (einmalig, Phase 0)

Auf dem Mac ist derzeit **kein Rust installiert**. Phase 0 richtet ein:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
rustup target add wasm32-unknown-unknown                           # Web-Ziel
cargo install trunk cargo-deny cargo-dist cargo-insta mdbook        # Werkzeuge
```

Editor-Empfehlung für Tobi zum Mitlesen: VS Code + `rust-analyzer`.

## 6. Arbeitsweise der Agenten

1. **Dieses Dokument lesen**, dann `CLAUDE.md` im Repo.
2. Pro Phase ein detaillierter Aufgabenplan unter `docs/plans/YYYY-MM-DD-phase-N.md`
   (writing-plans), erst nach Freigabe des Phasenziels.
3. Jede Aufgabe: Test zuerst, dann Implementierung, dann `cargo xtask ci` grün, dann Commit.
4. Commits klein und benannt nach Befehl/Feature (`engine: add task.move command`).
5. Branch je Phase, Merge in `main` nur mit grüner CI.
6. Abweichungen vom Plan: Abschnitt 9 ergänzen (Datum, Entscheidung, Grund).
7. Für Tobi: Jede Phase endet mit einem kurzen Demo-Schritt, den er ohne Rust-Kenntnis nachvollziehen
   kann (App starten, etwas tun, Datei ansehen).

## 7. Phasen

Jede Phase hat ein prüfbares Ergebnis („Definition of Done“). Reihenfolge ist verbindlich; die
Oberfläche kommt erst, wenn der Kern vollständig getestet ist.

### Phase 0 – Fundament (≈ 1 Session)

- Rust-Toolchain, Workspace mit allen Crates (leer, aber kompilierend), `xtask ci` inkl.
  Schichtprüfung, Clippy-Lints, `cargo deny`, GitHub-Repo (privat), GitHub Actions (Linux/macOS/
  Windows + WASM-Build), `CLAUDE.md`, Lizenzdatei, README.
- **DoD:** `cargo xtask ci` grün auf leeren Crates; CI läuft auf GitHub.

### Phase 1 – Kern: Modell, Befehle, Engine (≈ 3–4 Sessions)

- `core`: alle Entitäten aus 3.2, Validierung, Fehler-Typen, Clock/Id-Traits, `Classification`,
  `NetworkPolicy`.
- `engine`: `Session`, `CommandSpec`, Register, Events, Undo/Redo, Abfragen.
- Befehle (vollständige Basics): `project.create/update/archive`, `task.create/update/move/
  reparent/assign/set_status/set_progress/delete`, `milestone.create/update/reach/delete`,
  `dependency.add/remove`, `person.create/update`, `comment.add`, `undo`, `redo`.
- Abfragen: `tasks(filter, sort, group)`, `board(project)`, `overdue()`, `progress(project)`,
  `history(task)`.
- Tests: Unit je Befehl, Property-Tests mit Junk-Parametern für jeden Befehl, Snapshot-Tests
  der Abfragen.
- **DoD:** Headless-Beispiel in `stabsplan-cli` legt Projekt mit Aufgaben an und gibt Board als
  JSON aus. Testabdeckung des Kerns ≥ 90 %.

### Phase 2 – Speicherung (≈ 2 Sessions)

- `.stabsplan`-Container, Laden/Speichern, Formatversion + Migration, Autosave, atomares Schreiben.
- Verschlüsselung mit `age`, Hash-Kette im Event-Log, Prüfung beim Laden.
- CLI: `open`, `save`, `export --format json|csv|markdown`.
- Tests: Round-Trip, korrupte Dateien (Fuzz-artig), falsches Passwort, manipulierte Kette.
- **DoD:** CLI erstellt, verschlüsselt, lädt und prüft eine Datei; manipulierte Datei wird erkannt.

### Phase 3 – Desktop-Oberfläche (≈ 4–5 Sessions)

- `ui` mit egui: Projektübersicht, Liste, Board (Drag & Drop), Detailpanel, Befehlspalette,
  Theme, i18n de/en, Undo/Redo-Tasten, Dateidialoge.
- `apps/stabsplan` als eframe-App, Fenster-/Zustandspersistenz.
- **DoD:** Tobi legt in der App ein Projekt mit Aufgaben, Unteraufgaben und Meilensteinen an,
  verschiebt Karten im Board, speichert, schließt, öffnet wieder.

### Phase 4 – Web (≈ 1–2 Sessions)

- `apps/stabsplan-web` mit trunk; Upload/Download der Projektdatei, Zwischenstand in IndexedDB;
  Hinweisbanner zur VS-Eignung; Deploy auf GitHub Pages aus CI.
- **DoD:** Dieselbe App läuft im Browser mit derselben Datei wie auf dem Desktop.

### Phase 5 – Zeitplanung (≈ 2–3 Sessions)

- `schedule`: Graph, kritischer Pfad, Kalender. Gantt-Ansicht in `ui` mit Abhängigkeitspfeilen,
  Ziehen von Balken, Warnungen bei unhaltbaren Meilensteinen.
- **DoD:** Gantt zeigt kritischen Pfad; Verschieben einer Aufgabe verschiebt Nachfolger korrekt
  (durch Tests belegt).

### Phase 6 – KI: MCP und Assistent (≈ 3 Sessions)

- `mcp`: Server über `rmcp`, Tools aus 3.7, Doku als Ressource. Registrierung in Claude Code.
- `assistant`: Provider Ollama (Standard), Anthropic, OpenAI-kompatibel; Tool-Loop; Chat-Panel
  in `ui`; jede Aktion als markierter Undo-Schritt.
- **DoD:** Claude Code bearbeitet per MCP ein Projekt („verschiebe alle überfälligen Aufgaben um
  eine Woche“); der In-App-Assistent tut dasselbe mit einem lokalen Modell.

### Phase 7 – VS-Härtung (≈ 2 Sessions)

- Einstufung durchgängig: Vererbung, Anzeige, Export-Kennzeichnung, Herabstufungsschutz.
- `NetworkPolicy` hart bei VS-NfD, Audit-Ansicht (Verlauf gesamt, Export als signierter Bericht),
  keine Klartext-Temporärdateien, Sicherheits-Selbsttest in der App.
- Bedrohungsmodell als Dokument `docs/security.md`.
- **DoD:** Eine als VS-NfD eingestufte Datei kann weder unverschlüsselt gespeichert noch an einen
  externen KI-Provider gesendet werden; der Audit-Bericht listet jede Änderung.

### Phase 8 – Veröffentlichung (≈ 2 Sessions)

- `cargo-dist`-Releases (dmg, msi, AppImage), signiert soweit möglich; Web-Deploy; mdbook-Doku
  (Englisch + Deutsch); CONTRIBUTING, Code of Conduct, Security-Policy; Repo öffentlich.
- **DoD:** Version 0.1.0 installierbar auf drei Plattformen und im Browser erreichbar.

### Phase 9 – Team (Zukunft, nicht geplant)

`server`-Crate (axum), Event-Abgleich, Anmeldung. Erst nach 0.1.0 entscheiden.

## 8. Risiken und Gegenmaßnahmen

| Risiko | Gegenmaßnahme |
| --- | --- |
| egui-Web wirkt weniger „webtypisch“ (Textauswahl, Scrollverhalten) | Bewusst akzeptiert für v0.1; Kern ist UI-unabhängig, ein alternatives Frontend bliebe möglich |
| `age`/`zip`/`rmcp` kompilieren nicht für WASM | Features per Target; MCP nur nativ; Verschlüsselung im Web via `age` (pure Rust, WASM-fähig) – in Phase 0 per Smoke-Build verifizieren |
| Tobi kann Code nicht prüfen | Qualität ist maschinell erzwungen (`xtask ci`); jede Phase hat einen nutzerseitigen Demo-Schritt |
| Scope-Wachstum („nur noch Zeiterfassung“) | Nicht-Ziele in Abschnitt 1; neue Features nur mit Eintrag in Abschnitt 9 |
| VS-Anforderungen ändern sich | Einstufung ist erweiterbarer Enum; Krypto hinter Trait |

## 9. Entscheidungsprotokoll und Status

| Datum | Eintrag |
| --- | --- |
| 2026-10-14 | Plan erstellt. Lösung C gewählt (Einzelnutzer, Team vorbereitet). Arbeitstitel Stabsplan. Rust noch nicht installiert. |

| Phase | Status |
| --- | --- |
| 0 Fundament | offen |
| 1 Kern | offen |
| 2 Speicherung | offen |
| 3 Desktop-UI | offen |
| 4 Web | offen |
| 5 Zeitplanung | offen |
| 6 KI | offen |
| 7 VS-Härtung | offen |
| 8 Veröffentlichung | offen |

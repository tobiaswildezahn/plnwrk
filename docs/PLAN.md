# plnwrk – Architektur und Implementierungsplan

> **Dies ist das verbindliche Steuerdokument für alle Agenten und Sessions.**
> Vor jeder Arbeit lesen. Abweichungen hier eintragen, nicht stillschweigend umsetzen.
> Status der Phasen wird in Abschnitt 9 gepflegt.

Name: **plnwrk** (gesprochen „Planwerk“). Crates, Apps, Dateiendung und Repo heißen `plnwrk`.

## 1. Ziel und Leitplanken

**Was:** Eine Projekt- und Vorgangsmanagement-Software, die die Basics vollständig abbildet und
auf jedem Desktop (macOS, Windows, Linux) und im Browser läuft. Vorbild in Aufbau und
Qualitätsanspruch ist [RustGIS](https://rustgis.app).

**Wozu:** Modernes Projektmanagement in verkrustete Verwaltungsstrukturen bringen. Ein Vorgang
(Aktenzeichen, Frist, fester Ablauf) und ein Projekt (Ziel, Meilensteine, Team) sind dieselbe
Sache: ein Container mit Aufgaben. Deshalb gibt es **genau ein Datenmodell**, keine zwei
Strukturen. Der Einsatz in der Stabsarbeit ist eine Option, nicht der Zweck.

**Warum (Alleinstellung):**

1. **Datenhoheit** – jeder Arbeitsbereich (Projekte, Vorgänge, Vorlagen, Personen) ist eine Datei,
   die dem Nutzer gehört. Kein Konto, keine Cloud, keine Telemetrie.
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
plnwrk/
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
    ├── plnwrk/        Desktop (eframe nativ)
    ├── plnwrk-web/    Browser (eframe → wasm32, trunk)
    └── plnwrk-cli/    Kommandozeile: run, query, export, mcp
```

Abhängigkeitsrichtung strikt nach unten. `ui` kennt `engine`, nie umgekehrt.
`assistant` und `mcp` kennen nur `engine`, nie `ui`.

### 3.2 Domänenmodell (`core`)

Die vollständigen Basics des Projektmanagements:

| Entität | Felder (Kern) |
| --- | --- |
| `Project` | id, title, description (Markdown), **reference?** (Aktenzeichen, frei formatiert), **kind** (frei, z. B. „Projekt“, „SKA“, „Beschaffung“; dient Filter und Vorlagenwahl), owner?, start?, due?, status (draft/active/on_hold/done/archived), **template** (bool: dient als Vorlage), classification |
| `Task` | id, project, parent? (Unteraufgaben beliebig tief), title, description (Markdown), status (todo/in_progress/blocked/review/done), priority (low/normal/high/critical), assignee?, due?, start?, estimate?, progress 0–100, tags, classification |
| `Milestone` | id, project, title, date, status (open/reached/missed) |
| `Dependency` | from_task, to_task, kind (finish_to_start, start_to_start, finish_to_finish), lag_days |
| `Person` | id, name, email?, role, initials |
| `Comment` | id, task, author, created, body |
| `Classification` | open / vs_nfd (erweiterbar), gilt pro Projekt und pro Aufgabe; Aufgabe erbt vom Projekt, darf nur höher sein |

**Ein Modell für Projekt und Vorgang.** Es gibt keinen eigenen Vorgangstyp. Ein Vorgang ist ein
`Project` mit `reference` und `due`, meist aus einer Vorlage erzeugt (`project.instantiate`
kopiert Aufgaben, Meilensteine und Abhängigkeiten einer Vorlage mit relativen Terminen auf ein
Startdatum). „Bearbeitungsschritte“ eines Vorgangs sind Aufgaben, „Frist“ ist `due`,
„Bearbeiter“ ist `owner`. Die UI darf je nach `kind` „Vorgang“ oder „Projekt“ anzeigen; das ist
nur ein Label.

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

- **Dateiformat `.plnwrk`:** eine Datei je Arbeitsbereich (`Workspace`: beliebig viele Projekte
  und Vorgänge samt Vorlagen und Personen). ZIP-Container mit `manifest.json` (Formatversion, Einstufung,
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

1. **Übersicht** – Karten je Projekt/Vorgang mit Fortschritt, Frist, Überfälligem, nächstem Meilenstein; Filter nach `kind`; Vorlagen als eigener Bereich
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
- **MCP-Server:** `plnwrk-cli mcp --project p.plnwrk` über stdio, Crate `rmcp`. Tools:
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
- Befehle (vollständige Basics): `project.create/update/archive/instantiate` (aus Vorlage), `task.create/update/move/
  reparent/assign/set_status/set_progress/delete`, `milestone.create/update/reach/delete`,
  `dependency.add/remove`, `person.create/update`, `comment.add`, `undo`, `redo`.
- Abfragen: `tasks(filter, sort, group)`, `board(project)`, `overdue()`, `progress(project)`,
  `history(task)`.
- Tests: Unit je Befehl, Property-Tests mit Junk-Parametern für jeden Befehl, Snapshot-Tests
  der Abfragen.
- **DoD:** Headless-Beispiel in `plnwrk-cli` legt Projekt mit Aufgaben an und gibt Board als
  JSON aus. Testabdeckung des Kerns ≥ 90 %.

### Phase 2 – Speicherung (≈ 2 Sessions)

- `.plnwrk`-Container, Laden/Speichern, Formatversion + Migration, Autosave, atomares Schreiben.
- Verschlüsselung mit `age`, Hash-Kette im Event-Log, Prüfung beim Laden.
- CLI: `open`, `save`, `export --format json|csv|markdown`.
- Tests: Round-Trip, korrupte Dateien (Fuzz-artig), falsches Passwort, manipulierte Kette.
- **DoD:** CLI erstellt, verschlüsselt, lädt und prüft eine Datei; manipulierte Datei wird erkannt.

### Phase 3 – Desktop-Oberfläche (≈ 4–5 Sessions)

- `ui` mit egui: Projektübersicht, Liste, Board (Drag & Drop), Detailpanel, Befehlspalette,
  Theme, i18n de/en, Undo/Redo-Tasten, Dateidialoge.
- `apps/plnwrk` als eframe-App, Fenster-/Zustandspersistenz.
- **DoD:** Tobi legt in der App ein Projekt mit Aufgaben, Unteraufgaben und Meilensteinen an,
  erzeugt aus einer Vorlage einen Vorgang mit Aktenzeichen und Frist, verschiebt Karten im Board,
  speichert, schließt, öffnet wieder.

### Phase 4 – Web (≈ 1–2 Sessions)

- `apps/plnwrk-web` mit trunk; Upload/Download der Projektdatei, Zwischenstand in IndexedDB;
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
| 2026-10-14 | Plan erstellt. Lösung C gewählt (Einzelnutzer, Team vorbereitet). Rust noch nicht installiert. |
| 2026-10-14 | Name **plnwrk** (statt Stabsplan: Stabsarbeit ist Option, nicht Zweck). Projekt und Vorgang sind **ein** Modell (`Project` mit `reference`, `kind`, `due`, Vorlagen); keine zweite Struktur. |
| 2026-10-14 | Phase 0: Rust **1.99.0** (in `rust-toolchain.toml` fixiert). Paketnamen `plnwrk-*` (ein Crate darf nicht `core` heißen). Zusätzlich zur Plan-Fassung: Reinheitsprüfung für core/engine in `xtask` (kein fs/net/env/thread/clock/rand) und Clippy-Lauf für `wasm32`. Minimale egui-App schon in Phase 0, damit Desktop- und Web-Pfad ab Tag 1 in CI geprüft werden. |
| 2026-10-14 | WASM-Risiko (§8) geprüft per Wegwerf-Build: `age` 0.12, `zip` 9 (ohne Default-Features, `deflate`), `ulid` 3, `jiff` 0.2, `blake3` 1 bauen für `wasm32-unknown-unknown`. **Auflagen:** (1) Web-App muss `getrandom` 0.2 mit Feature `js` und 0.4 mit `wasm_js` aktivieren (age zieht beides). (2) `ulid` 3 hat kein `Ulid::new()` ohne Zufallsquelle – passt zur Regel „deterministischer Kern“: IDs per `Ulid::from_parts(zeit, zufall)` aus Clock-/Id-Traits, die die Apps liefern. `jiff::Timestamp::now()` nie im Kern verwenden. |
| 2026-10-14 | Review Phase 0 (unabhängiger Agent) → Schranke gehärtet: Absturz-Lints `forbid` + zusätzlich als `-F`-Flags in `xtask ci` (greifen auch ohne `[lints] workspace = true`); neue Lints `string_slice`, `unreachable`, `exit`, `allow_attributes_without_reason`; Reinheit primär über `disallowed-methods/types` in `crates/{core,engine}/clippy.toml`, Textsuche nur Rückfallebene; Positivliste externer Abhängigkeiten für core/engine; Workspace-Crates per Mitgliedschaft statt Namenspräfix; wasm32-Prüfung für alle Crates außer `NATIVE_ONLY` plus echter WASM-Build; `--locked` überall, CI prüft Cargo.lock; CI-Werkzeuge versioniert, Ubuntu 24.04 fixiert. |
| 2026-10-14 | Bewusste Abweichungen Phase 0: cargo-dist, mdbook, cargo-insta werden erst in der Phase installiert, die sie braucht. `deny.toml` erlaubt nur tatsächlich genutzte Lizenzen (strenger als Plan). Der eine UI-Text der Startseite ist bis zur i18n in Phase 3 deutsch fest verdrahtet. |
| 2026-10-14 | Phase 1: **Eine Datei = ein Arbeitsbereich** mit vielen Projekten/Vorgängen, Vorlagen und Personen (statt „ein Projekt = eine Datei“; Vorgänge sind zahlreich und klein, Vorlagen und Personen werden geteilt). §1 und §3.4 angepasst. |
| 2026-10-14 | Phase 1: Events sind `Put{before, after}` / `Remove{before}` – ohne Nachschlagen umkehrbar. Undo/Redo erzeugen kompensierende Changes, das Log bleibt append-only; der Undo-Stapel wird nicht gespeichert. Nach **jedem** Befehl prüft `Workspace::check` alle Invarianten; bei Verstoß vollständiger Rollback (Sicherheitsnetz gegen vergessene Einzelprüfungen). `Outcome.change` ist `Option<u64>` (kein Eintrag, wenn nichts geändert wurde). |
| 2026-10-14 | Phase 1: Meilenstein „verpasst“ wird aus Datum und Status abgeleitet, nicht gespeichert. `Project.kind` optional, Default-Status `active`. Reihenfolge per fraktionalem Rang (seit Review: Basis 62 mit ganzzahligem Kopf), damit ein Verschieben normalerweise genau eine Aufgabe ändert. Parameter strikt: unbekannte Schlüssel → Fehler, `null` leert. |
| 2026-10-14 | Phase 1 Technik: `ulid` ohne `serde`-Feature (Fehler in ulid 3 ohne std), eigene serde-Implementierung der ID-Typen. `SystemClock`/`RandomIds` (monoton steigend) liegen vorerst in `plnwrk-cli`; ziehen in einen gemeinsamen Plattform-Crate um, sobald Desktop/Web sie brauchen. clap per Builder-API: Proc-Makros, die verbotene Lints per `#[allow]` abschalten (clap-derive), sind mit `forbid` unvereinbar – Regel für alle künftigen Makros. |
| 2026-10-14 | Phase 1 Qualität: Abdeckung core+engine ≥ 90 % als Schritt in `cargo xtask ci` (cargo-llvm-cov; Stand 95 %). Müll-Parameter-Test als typisiertes Mutations-Fuzzing aller 23 Befehle und 9 Abfragen; jeder Befehl muss ≥ 5 echte Erfolge erreichen, sonst schlägt der Test fehl (verhindert, dass er unbemerkt nur Parameterprüfungen testet). |
| 2026-10-14 | Review Phase 1 (unabhängiger Agent) → behoben: **Rang** jetzt Standard-„fractional indexing“ mit ganzzahligem Kopf variabler Länge, Basis 62 (10 000 Anhängungen ≤ 4 Zeichen; vorher Abbruch nach ~1 500); ist eine Lücke erschöpft, verteilt der Befehl die Reihenfolge des Projekts neu (im selben Undo-Schritt). **Vorlagen**: Termine, die beim Verschieben aus dem Kalender fallen, führen zu einem Fehler statt stillem Verlust. **Aufgabenlisten** ohne Projektfilter ohne Vorlagen und Archiv; Aufgaben/Meilensteine abgeschlossener Projekte nicht mehr überfällig. **Leistung**: Regelprüfung und Abfragen linear (10 000 Aufgaben: check 1,8 ms, Board 2 ms; vorher 188/353 ms). **`Tx::insert`**: Anlegen überschreibt nie. |
| 2026-10-14 | Review Phase 1 → Regeln ergänzt: **Einstufung vererbt sich auch über Elternaufgaben** (Unteraufgabe nie niedriger als Elternaufgabe). **Unsichtbare Steuerzeichen** (Nullbreite, Schreibrichtungs-Umkehr, BOM) in allen Texten verboten („Trojan Source“). Auswahllisten der Befehle aus den Enums erzeugt (`NAMES`). Initialen folgen einer Namensänderung, wenn sie abgeleitet waren. |
| 2026-10-14 | Review Phase 1 → Tests: Rollback-, Undo- und Redo-Sicherheitsnetze jetzt getestet und per **Mutationsprobe** belegt (6 absichtlich eingebaute Fehler werden alle erkannt); modellbasierter Sequenztest (zufällige Befehlsfolgen mit Undo/Redo gegen Zustandsmodell); Fuzzer prüft auch `redo` und die Undo/Redo-Stapel. Bekannte Schwäche: In-Datei-Testmodule zählen zur Abdeckung (Werkzeuggrenze, stabiles Rust kennt kein `coverage(off)`). |
| 2026-10-14 | Nachreview Phase 1 → **Regel: Jeder Text, den ein Assistent lesen kann, wird auf unsichtbare Zeichen geprüft** (Unicode-Kategorie Cf, Zeilen-/Absatztrenner, Tag-Zeichen – Träger für versteckte KI-Anweisungen). Erlaubt bleiben, was echte Schrift braucht: weicher Trennstrich, ZWJ/ZWNJ (Emoji, persisch, indisch), LRM/RLM (hebräische/arabische Namen). |
| 2026-10-14 | Nachreview Phase 1 → **Eine Regel für „überfällig“/„verpasst“** (`deadlines_count`): gilt nicht für Vorlagen, abgeschlossene und archivierte Projekte – in Liste, Board, Übersicht, Detail und `overdue` gleich. Vorlagen mit Terminen, aber ohne Startdatum, werden beim Erzeugen abgelehnt (vorher wurden Termine still verworfen). Übersicht/Überfälliges gruppieren Aufgaben und Meilensteine einmal je Projekt (linear auch bei vielen Projekten). |
| 2026-10-14 | Lehre zur Arbeitsweise: Jede Behauptung „Test fängt X“ wird per Mutationsprobe belegt; zwei Tests waren beim ersten Versuch nicht in der Datei gelandet und fielen nur dadurch auf. Kandidat für Phase 2: `cargo-mutants` als nicht-blockierender Schritt `cargo xtask mutants` für rank, place/spread und Abfragen. |
| 2026-10-14 | Phase 2: **Dateiformat v1** – immer ZIP mit lesbarem `manifest.json` (Format, Version, Einstufung, verschlüsselt ja/nein, Programm). Unverschlüsselt daneben `workspace.json` + `log.jsonl` (BLAKE3-Hash-Kette, Kopf-Hash im Manifest); verschlüsselt liegt dieses innere ZIP als `payload.age` (age, scrypt-Passphrase) vor, das äußere Manifest nennt dann weder Anzahl noch Hash der Änderungen. Laden prüft Version, Kette, **Replay des Logs = gespeicherter Zustand**, alle Invarianten und Einstufung (VS-NfD nur verschlüsselt, Kopf ≠ Inhalt → Fehler). Limits gegen ZIP-Bomben (256 MiB, 64 Einträge). |
| 2026-10-14 | Phase 2: `plnwrk-store` arbeitet auf Bytes und baut für den Browser; atomares Schreiben (`store::file`) nur nativ. Autosave: Pfad-Konvention `.<name>.autosave.plnwrk` steht, die Auslöse-Logik (wann gesichert wird) gehört in die Oberfläche (Phase 3). CSV mit `;` und deutschen Spaltennamen (Excel in deutscher Umgebung), Formel-Injektion entschärft; Markdown escaped. Abdeckungsschranke umfasst jetzt auch `plnwrk-store`. Ehrliche Grenze: Eine unverschlüsselte Datei kann bei vollständiger Neuberechnung von Kette und Zustand unbemerkt verändert werden – Schutz dagegen ist die Signatur in Phase 7. |
| 2026-10-14 | Phase 2 fand einen **Fehler aus Phase 1**: `Entity` nutzte im JSON das Typfeld `kind`, das mit `Project.kind` kollidiert – Projekte ließen sich schreiben, aber nicht einlesen. Typfeld jetzt `entity`; Round-Trip-Test über alle Entitätsarten (per Mutationsprobe belegt). |
| 2026-10-14 | Security-Review Phase 2 → **Blocker behoben: Einstufung ist „klebrig“.** Die Einstufung einer Datei ist das Maximum über den aktuellen Stand **und die gesamte Änderungsgeschichte** (`classification_of`): Wurde je VS-NfD-Inhalt gespeichert, bleibt die Datei VS-NfD, auch nach Löschen oder Herabstufen – wie eine Papierakte. Beim Laden muss der Kopf exakt diesem Wert entsprechen. Echte Herabstufung = eigener, dokumentierter Vorgang ohne Geschichte (Phase 7). |
| 2026-10-14 | Security-Review Phase 2 → Format gehärtet: Hash-Kette über die **gespeicherten Bytes** jeder Zeile (keine Neu-Serialisierung), keine Leerzeilen, `deny_unknown_fields` auf allen gespeicherten Strukturen; Container nur mit exakt den erwarteten Einträgen, doppelte ZIP-Namen erkannt (Abgleich mit dem Endverzeichnis), lesbarer Kopf verschlüsselter Dateien ohne Inhaltsangaben; scrypt-Faktor **fest 18** (256 MiB) statt geräteabhängig, Lesen höchstens 20 (Browser 18); Passphrase Unicode-normalisiert (NFC); verschlüsselter Inhalt auf 4-KiB-Blöcke aufgefüllt; Export-JSON mit Kennzeichen, CSV mit BOM und Spalte Einstufung. |
| 2026-10-14 | Security-Review Phase 2 → Dateien und CLI: Zwischendatei exklusiv unter Zufallsnamen (kein Umlenken per Symlink), neue Dateien 0600, bestehende behalten ihre Rechte, Ordner wird synchronisiert; `new` überschreibt nie (atomares Anlegen); ändernde CLI-Befehle sperren die Datei (`.<name>.lock`); neue Passphrasen ≥ 12 Zeichen; Warnung, wenn eine Passphrase für eine unverschlüsselte Datei gesetzt ist; „Änderung NICHT gespeichert“ eindeutig gemeldet. Kryptobibliotheken auch in Debug-Builds optimiert. |
| 2026-10-14 | Bekannte Grenzen (bewusst): Ersetzen einer verschlüsselten Datei durch eine **ältere Fassung mit derselben Passphrase** wird nicht erkannt (Rollback; Abhilfe erst mit Signatur/Zähler außerhalb der Datei). „Reproduzierbares Speichern“ gilt nur innerhalb eines Builds (Kompressions-Backend). Offene Frage für Phase 4: Soll der Browser VS-NfD-Dateien grundsätzlich nicht öffnen? Vorschlag: `cargo xtask mutants` (cargo-mutants, nicht blockierend) für store und engine. |
| 2026-10-14 | Nachreview Phase 2 (kein Blocker) → behoben: **Exporte folgen der Datei-Einstufung inkl. Geschichte**; **Umhängen behält eine geerbte Einstufung** (eine Aufgabe wird nie still herabgestuft). ZIP-Struktur wird **selbst abgelaufen** (Einträge lückenlos ab Byte 0, Verzeichnis, Endeintrag ohne Kommentar – nichts davor, dazwischen, dahinter; Anzahl geprüft, bevor Speicher belegt wird). `workspace.json` muss **byte-genau** dem Replay entsprechen; Protokollzeilen exakt im geschriebenen Format; Parameter jeder geloggten Änderung gegen die Befehlsbeschreibung geprüft; Manifest ohne doppelte Schlüssel. CLI löst Symlinks auf (Sperre und Speichern treffen die echte Datei), `new` verlässt sich allein auf atomares Nie-Überschreiben (auch auf Dateisystemen ohne harte Links mit Aufräumen und 0600), Passphrasenlänge nach Unicode-Normalisierung, präzisere Warnungen. Alle 17 Mutationen der Lücken geprüft; „ZIP-Kommentar“-Prüfung ist eine gleichwertige Zusatzsicherung. |
| 2026-10-14 | **Entscheidung Tobi (STATUS 4.4): früh etwas Zeigbares**, volle VS-Umsetzung später; Test auf dem eigenen Mac mit ausgedachten Daten. Phase 3 wird geteilt: **3a Desktop-MVP** (Dateien, Übersicht, Liste, Board mit Drag & Drop, Meilensteine, Detailansicht mit Kommentaren und Verlauf, Vorlagen, Personen, Rückgängig) und **3b** (Befehlspalette, Englisch, Inline-Bearbeitung in der Liste, Autosave/Wiederherstellung, Verschlüsseln aus der App, Theme-Umschalter, Fensterzustand je Datei). Plan: `docs/plans/2026-10-14-phase-3.md`. |
| 2026-10-14 | Phase 3a umgesetzt: neuer Crate **`plnwrk-platform`** (Schicht 2: echte Uhr, Zufalls-IDs; im Browser über JavaScript) statt Kopie in jeder App; `Document::from_session`/`into_session` im store. Oberfläche liest typisiert über die Engine-Abfragen und beschreibt Änderungen als *Intents*, die `PlnwrkApp` nach dem Zeichnen über `Session::run` ausführt. Dateidialoge über einen `Host`-Trait (Desktop: **rfd**, nur xdg-portal unter Linux), damit `ui` für wasm32 baut. Oberflächentests ohne Fenster mit **egui_kittest** über den Barrierefreiheitsbaum (Felder per `labelled_by` mit ihrer Beschriftung verknüpft). `cargo xtask app` baut `target/plnwrk.app`. Abweichung: UI-Texte vorerst nur Deutsch (überwiegend zentral in `crates/ui/src/text.rs`, einzelne Dialog-Hinweise noch in den Ansichten); Englisch in 3b. Zeichen nur aus den eingebauten Schriften (Ubuntu, Emoji-Fonts) – ◆ ● ↑ ✕ fehlen dort. |
| 2026-10-14 | Review Phase 3a → behoben: **Cmd+Q** beendete auf macOS ohne Rückfrage (winit-Standardmenü ruft `terminate:`) → Standardmenü aus, Cmd+Q läuft über die Rückfrage; Beenden über Dock/Abmelden schreibt eine Notkopie `.<name>.autosave.plnwrk` neben die Datei. **Getippter, noch nicht übernommener Text** zählt als ungespeichert und wird vor jeder Dateiaktion übernommen (falsches Datum stoppt mit Meldung). Entwurf folgt Änderungen von außen feldweise, ohne Getipptes zu verlieren; Felder je Aufgabe getrennt; Kommentar-/Unteraufgaben-Eingabe bleibt bei ihrer Aufgabe. Kürzel ruhen, solange ein Dialog offen ist. „Speichern unter“ und Sperre auf aufgelöstem Pfad; dieselbe Datei erneut öffnen meldet „bereits geöffnet“. Falsche Passphrase fragt erneut, eingegebene Passphrase wird beim Schließen gelöscht. Projektformular folgt Rückgängig. 12 Mutationsproben, alle gefangen. Offen (3b): Wiederherstellen der Notkopie, Doppelklick auf `.plnwrk` im Finder, Öffnen aus schreibgeschützten Ordnern (Sperrdatei), Verschlüsseln blockiert ≈ 1 s. |
| 2026-10-14 | Nachreview Phase 3a (kein Blocker) → behoben: Getipptes blockiert nie mehr Speichern/Schließen – ungültige Daten und abgelehnte Eingaben (leerer Titel) werden zurückgesetzt und in einer Meldung genannt, der Rest wird übernommen. **Notkopien** (Beenden über Dock/Abmelden) gehen jetzt auch für unbenannte Dateien in einen sichtbaren Ordner (`~/Library/Application Support/plnwrk/Notkopien`, Windows `%APPDATA%`, sonst `~/.local/share`) und stehen auf der Startseite zum Öffnen. „Öffnen“ wählt erst die Datei, dann die Rückfrage; dieselbe Datei erneut öffnen fragt nicht. Fenstertitel zeigt ● auch für Getipptes. 9 weitere Mutationsproben, alle gefangen. Bewusst offen: Cmd+Q ruht, solange ein Dialog offen ist; ungespeicherte Eingaben im Projektformular (eigener „Übernehmen“-Knopf) zählen nicht als ungespeichert; ohne macOS-Standardmenü fehlen Cmd+H/Cmd+M. |
| 2026-10-14 | **Entscheidung Tobi: Windows-CI nur vor dem Merge.** Linux, macOS und Web-Bundle laufen bei jedem Push; Windows (≈ 15 min) erst, wenn der PR das Label `merge` trägt. Merge nur, wenn auch Windows grün ist. Cmd+Q-Rückfrage von Tobi in der echten App bestätigt. |

| Phase | Status |
| --- | --- |
| 0 Fundament | fertig (2026-10-14) |
| 1 Kern | fertig (2026-10-14, PR #2, zwei Reviews) |
| 2 Speicherung | fertig (2026-10-14, PR #3, Security-Review + Nachreview, CI grün auf Linux/macOS/Windows/Web) |
| 3 Desktop-UI | 3a Desktop-MVP umgesetzt, PR offen (2026-10-14); 3b offen |
| 4 Web | offen |
| 5 Zeitplanung | offen |
| 6 KI | offen |
| 7 VS-Härtung | offen |
| 8 Veröffentlichung | offen |

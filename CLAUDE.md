# plnwrk – Agenten-Einstieg

**Zuerst lesen: [docs/STATUS.md](docs/STATUS.md)** (aktueller Stand, offene Punkte, nächste
Schritte), **dann [docs/PLAN.md](docs/PLAN.md).** Es ist das verbindliche Steuerdokument
(Architektur, Regeln, Phasen, Entscheidungsprotokoll). Nichts bauen, was dort nicht vorgesehen
ist; Abweichungen dort in Abschnitt 9 eintragen.

## Kurzregeln

- Nur Rust. Schichtung strikt (core → engine → store/schedule → assistant/mcp → ui → apps).
- Jede Nutzeraktion ist ein `CommandSpec`; nur `Session::run` ändert Zustand.
- Kein `unwrap`/`expect`/`panic!` in Produktionscode. Eingaben sind feindlich.
- TDD: Test zuerst. Jeder Befehl zusätzlich mit Junk-Parametern getestet.
- Fertig heißt: `cargo xtask ci` grün (fmt, clippy -D warnings, Tests, Schichtprüfung, deny, WASM).
- Kein Netzzugriff ohne `NetworkPolicy`. Keine Telemetrie.
- Code/Commits Englisch, UI i18n de+en, Doku im Repo Deutsch.

## Befehle

```sh
source ~/.cargo/env                     # falls cargo nicht im PATH ist
cargo xtask ci                          # die Schranke; muss grün sein
cargo xtask layers | purity             # Einzelprüfungen
cargo run -p plnwrk-cli -- demo         # Beispiel-Arbeitsbereich als JSON
cargo run -p plnwrk-cli -- new a.plnwrk --sample   # Datei anlegen (run/query/verify/export/encrypt)
cargo run -p plnwrk-cli -- commands     # alle Befehle mit JSON-Schema
cargo run -p plnwrk                     # Desktop-App (Entwicklung)
cargo xtask app                         # Release-Build + target/plnwrk.app (Doppelklick)
cd apps/plnwrk-web && trunk serve       # Browser-Version
```

- Neuer Crate → Eintrag in der Schichttabelle `xtask/src/layers.rs`. Jeder Crate wird für
  wasm32 geprüft, außer er steht in `NATIVE_ONLY` (`xtask/src/ci.rs`).
- core/engine: neue externe Abhängigkeit nur nach Eintrag in `PURE_EXTERNAL`
  (`xtask/src/layers.rs`); verbotene Aufrufe stehen in `crates/{core,engine}/clippy.toml`.
- Absturz-Lints sind `forbid`: `#[allow]` hilft nicht. Lösung ist immer echte Fehlerbehandlung.
- Jedes `#[allow]`/`#[expect]` braucht `reason = "…"`.
- Integrationstests (`crates/*/tests/*.rs`): Inhalt in `#[cfg(test)] mod … { }` packen, sonst
  gelten Hilfsfunktionen für Clippy als Produktionscode (`unwrap`/`panic!` verboten).
- Neue Befehle: `CommandSpec` im passenden `crates/engine/src/commands/*.rs`, Eintrag in
  `REGISTRY` (`command.rs`), Verhaltenstest in `crates/engine/tests/commands.rs`. Der
  Müll-Parameter-Test (`tests/junk.rs`) erfasst ihn automatisch.
- Abfragen: `QuerySpec` in `crates/engine/src/query/mod.rs`; Snapshots mit
  `INSTA_UPDATE=always cargo test -p plnwrk-engine --test queries` erneuern und Diff prüfen.
- Dateiformat (`crates/store`): jede Änderung am Format braucht eine neue `manifest::VERSION`
  und einen Migrationsschritt; Tests in `crates/store/tests/store.rs` decken Manipulationen ab.
  Neue Felder im Modell immer mit `#[serde(default, skip_serializing_if = …)]`, sonst passen
  alte Dateien nicht mehr byte-genau zu ihrem Protokoll. Echte Dateien älterer Versionen liegen in
  `crates/store/tests/fixtures/` und müssen immer öffnen. Datenumbauten laufen als Befehl
  (`workspace.upgrade`), nie durch Umschreiben der Geschichte; stillgelegte Befehle bleiben in
  `LEGACY` (`crates/engine/src/command.rs`), damit alte Protokolle prüfbar bleiben.
- Skript-Ersetzungen in Quelltexten immer mit `assert old in s` absichern (nach `cargo fmt`
  passen Muster oft nicht mehr) und Testbehauptungen per Mutationsprobe belegen.
- Kein Proc-Makro, das verbotene Lints per `#[allow]` abschaltet (z. B. clap-derive).
- Versionen von Abhängigkeiten stehen nur im Workspace-`Cargo.toml` (`[workspace.dependencies]`).
- UI-Code ist über `PlnwrkApp::show(&mut egui::Ui)` ohne Fenster testbar: `crates/ui/src/tests.rs`
  klickt und tippt mit **egui_kittest** über den Barrierefreiheitsbaum. Eingabefelder darum
  immer per `.labelled_by(label.id)` mit ihrer Beschriftung verknüpfen (hilft Screenreadern
  und Tests). Ansichten ändern nie Daten selbst, sondern legen `Intent`s ab.
- Jede neue Eingabestelle (Feld, das erst bei Fokusverlust übernimmt) muss in
  `PlnwrkApp::has_unsaved_changes` gezählt und in `flush_draft` übernommen werden – sonst gehen
  getippte Werte bei Speichern/Schließen verloren (drei Review-Funde hatten genau diese Ursache).
- UI-Texte gehören nach `crates/ui/src/text.rs`: feste Texte als Feld in `struct Texts` mit Wert
  in `DE` **und** `EN` (Zugriff `text::t().feld`), Texte mit Werten als Funktion mit
  `pick(deutsch, englisch)`. Ein Wächtertest verbietet deutsche Textliterale anderswo, ein
  englischer Rundgangtest sucht deutsche Wörter. Schrift ist Fira Sans (Teilmenge in
  `crates/ui/assets/fonts/`), dahinter die egui-Schriften; nur Zeichen verwenden, die eine davon
  hat (z. B. ♦ • statt ◆ ✕). Symbole in Knöpfen sind Vektoren (`design::Icon`), keine Emoji.
- **Gestaltung** (Phase 9, `crates/ui/src/design/`): Farben nur als Rolle aus
  `design::palette(ui)` – ein Wächtertest verbietet `Color32::from_*` und Farbkonstanten
  außerhalb `design/`, ein Kontrasttest prüft jedes Paar (WCAG AA). Knöpfe über
  `design::widgets::button(ui, Kind::…, text)`: eine Hauptaktion `Primary`, Löschen/Verwerfen
  `Destructive` (nie direkt neben `Primary`); Symbolknöpfe `icon_button(ui, Icon::…, name)`
  (mindestens 24 × 24). Neue Bausteine setzen `widget_info`.
- Browser: Logik für Dateien/Zwischenstand gehört in `plnwrk-ui` (Host-Methoden, nativ mit
  `TestBrowser` testbar), nur die JavaScript-Anbindung nach `apps/plnwrk-web/src/host.rs`.
  Nichts in `plnwrk-ui` darf im Browser anhalten: `std::process::id` ist dort per Clippy
  verboten (`plnwrk_platform::instance()` nehmen); `std::fs`/Threads nur hinter
  `cfg(not(target_arch = "wasm32"))`. Die CI baut das Web-Paket nur – ausprobieren mit
  `trunk serve` (Browser-Vorschau `plnwrk-web`).
- Sperren und Prüfungen (z. B. VS-NfD im Browser) **vor** `Session::run`, nie durch
  Rückgängig danach: Das Protokoll hängt nur an, ein zurückgenommener Befehl bleibt im Verlauf
  und zählt für die Einstufung der Datei (`plnwrk_store::classification_of`).
- Zeitplanung: Rechnen nur in `plnwrk-schedule` (rein, Daten als Parameter), immer mit
  `Calendar::standard()` (jeder Tag zählt, Entscheidung 2026-10-15), nie mit einem fest
  gewählten Kalender. Oberfläche und MCP berechnen Schübe über `Reschedule` und schicken
  **einen** `task.reschedule` (ein Rückgängig-Schritt).
  Browser-Vorschau zeigt nach Änderungen alte Stände? Seite mit `?v=<n>` neu laden (Cache).
- MCP-Server (`crates/mcp`, `plnwrk-cli mcp`, Anleitung `docs/mcp.md`): eigenes JSON-RPC über
  stdio (kein `rmcp`), Protokollteil `Server::handle` ohne Prozess testbar. Nichts VS-NfD und
  nichts Verschlüsseltes über MCP – geprüft vor dem Lesen, vor `Session::run` und vor dem
  Speichern. Änderungen mit `Origin::Assistant`. Gemeinsame Logik mit der Oberfläche liegt in
  `plnwrk_engine::params::asks_for_classified` und `plnwrk_schedule::Reschedule`, nie doppelt.
  Neue Befehle sind über `run_command` automatisch erreichbar.
- Neuer Befehl im Register → Eintrag in der Befehlspalette (`crates/ui/src/palette.rs`) oder
  mit Begründung in `palette::ELSEWHERE`; ein Test prüft das.
- kittest: Untermenü-Knöpfe heißen „Name⏵“ (Hilfsfunktion `submenu` in `tests.rs`); Tippen in
  Textfelder über `Event::Text`, nicht `key_press`; die Testumgebung startet im Dunkelmodus.
- egui/eframe 0.36: `eframe::App::ui(&mut self, ui, frame)` ist die Zeichenmethode (nicht `update`),
  Panels mit `.show(ui, …)`; Tests ohne Fenster müssen `output.textures_delta.clear()` aufrufen.
- Browser-Vorschau im Claude-Desktop: Eintrag `plnwrk-web` in `~/.claude/launch.json` (Port 8087).

## Arbeitsablauf

1. Phasenplan unter `docs/plans/` lesen oder (nach Freigabe) anlegen.
2. Kleine Commits: `<crate>: <was>` (z. B. `engine: add task.move command`).
   `Cargo.lock` gehört immer mit in den Commit, wenn sich Abhängigkeiten geändert haben: Die
   lokale Schranke prüft den Arbeitsordner, die CI mit `--locked` den Commit (zweimal rot
   wegen vergessener Lock-Datei). Vor dem Push: `git status` muss leer sein.
3. Branch je Phase, Merge nur mit grüner CI. Windows-CI läuft nur vor dem Merge: Label `merge`
   an den PR (`gh pr edit <nr> --add-label merge`), grün abwarten, dann mergen.
4. Phasenende: Demo-Schritt für Tobi (kein Rust-Wissen voraussetzen) und Status in PLAN.md §9.

## Der Mensch im Projekt

Tobi hat keine Rust-Erfahrung und prüft Ergebnisse über Verhalten, nicht über Code.
Erklärungen kurz, auf Deutsch, ohne Fachjargon ohne Erklärung. Qualität wird durch die
Schranke `cargo xtask ci` garantiert, nicht durch Zusicherungen.

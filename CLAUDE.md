# plnwrk – Agenten-Einstieg

**Zuerst lesen: [docs/PLAN.md](docs/PLAN.md).** Es ist das verbindliche Steuerdokument
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
cargo run -p plnwrk                     # Desktop-App
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
- Skript-Ersetzungen in Quelltexten immer mit `assert old in s` absichern (nach `cargo fmt`
  passen Muster oft nicht mehr) und Testbehauptungen per Mutationsprobe belegen.
- Kein Proc-Makro, das verbotene Lints per `#[allow]` abschaltet (z. B. clap-derive).
- Versionen von Abhängigkeiten stehen nur im Workspace-`Cargo.toml` (`[workspace.dependencies]`).
- UI-Code ist über `PlnwrkApp::show(&mut egui::Ui)` ohne Fenster testbar (siehe `crates/ui`).
- egui/eframe 0.36: `eframe::App::ui(&mut self, ui, frame)` ist die Zeichenmethode (nicht `update`),
  Panels mit `.show(ui, …)`; Tests ohne Fenster müssen `output.textures_delta.clear()` aufrufen.
- Browser-Vorschau im Claude-Desktop: Eintrag `plnwrk-web` in `~/.claude/launch.json` (Port 8087).

## Arbeitsablauf

1. Phasenplan unter `docs/plans/` lesen oder (nach Freigabe) anlegen.
2. Kleine Commits: `<crate>: <was>` (z. B. `engine: add task.move command`).
3. Branch je Phase, Merge nur mit grüner CI.
4. Phasenende: Demo-Schritt für Tobi (kein Rust-Wissen voraussetzen) und Status in PLAN.md §9.

## Der Mensch im Projekt

Tobi hat keine Rust-Erfahrung und prüft Ergebnisse über Verhalten, nicht über Code.
Erklärungen kurz, auf Deutsch, ohne Fachjargon ohne Erklärung. Qualität wird durch die
Schranke `cargo xtask ci` garantiert, nicht durch Zusicherungen.

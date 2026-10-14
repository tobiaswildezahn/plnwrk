# Stabsplan – Agenten-Einstieg

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

## Arbeitsablauf

1. Phasenplan unter `docs/plans/` lesen oder (nach Freigabe) anlegen.
2. Kleine Commits: `<crate>: <was>` (z. B. `engine: add task.move command`).
3. Branch je Phase, Merge nur mit grüner CI.
4. Phasenende: Demo-Schritt für Tobi (kein Rust-Wissen voraussetzen) und Status in PLAN.md §9.

## Der Mensch im Projekt

Tobi hat keine Rust-Erfahrung und prüft Ergebnisse über Verhalten, nicht über Code.
Erklärungen kurz, auf Deutsch, ohne Fachjargon ohne Erklärung. Qualität wird durch die
Schranke `cargo xtask ci` garantiert, nicht durch Zusicherungen.

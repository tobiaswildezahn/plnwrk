# plnwrk

**plnwrk** (gesprochen „Planwerk“) ist eine Software für Projekt- und Vorgangsmanagement,
komplett in Rust. Sie läuft auf jedem Desktop und im Browser, speichert jedes Projekt als eigene
Datei, funktioniert ohne Netz und ist vollständig durch KI steuerbar.

Ziel: modernes Projektmanagement in gewachsene Verwaltungsstrukturen bringen. Ein Vorgang
(Aktenzeichen, Frist, fester Ablauf) und ein Projekt sind dabei dasselbe – ein Datenmodell.

> **Status:** Phase 1 (Kern) fertig: Datenmodell, 23 Befehle mit Undo/Redo und Revisionsspur,
> 9 Abfragen – bedienbar über die Kommandozeile. Oberfläche folgt. Fahrplan: [docs/PLAN.md](docs/PLAN.md).

## Starten

Voraussetzung: [Rust](https://rustup.rs) (die Version legt `rust-toolchain.toml` fest).

```sh
cargo run -p plnwrk                                  # Desktop-App
cd apps/plnwrk-web && trunk serve                    # Browser-Version (http://127.0.0.1:8080)
cargo run -p plnwrk-cli -- demo                      # Beispiel als JSON
cargo run -p plnwrk-cli -- commands                  # alle Befehle mit Parametern
```

## Qualität

Eine einzige Schranke prüft alles: Formatierung, Linter (`clippy -D warnings`, nativ und für
WebAssembly), Tests, Schichtregeln, Reinheit des Kerns und Lizenzen/Sicherheitsmeldungen der
Abhängigkeiten.

```sh
cargo install --locked cargo-deny cargo-llvm-cov   # einmalig
cargo xtask ci
```

## Aufbau

| Schicht | Crate | Aufgabe |
| --- | --- | --- |
| 0 | `plnwrk-core` | Datenmodell, Regeln |
| 1 | `plnwrk-engine` | Befehle, Undo, Events, Abfragen |
| 2 | `plnwrk-store`, `plnwrk-schedule` | Dateiformat, Zeitplanung |
| 3 | `plnwrk-assistant`, `plnwrk-mcp` | KI-Assistent, MCP-Server |
| 4 | `plnwrk-ui` | Oberfläche (egui) |
| Apps | `plnwrk`, `plnwrk-web`, `plnwrk-cli` | Desktop, Browser, Kommandozeile |

Ein Crate darf nur Crates aus tieferen Schichten verwenden; `cargo xtask ci` prüft das.

## Lizenz

[Apache-2.0](LICENSE)

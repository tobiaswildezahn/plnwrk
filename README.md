# plnwrk

**plnwrk** (gesprochen „Planwerk“) ist eine Software für Projekt- und Vorgangsmanagement,
komplett in Rust. Sie läuft auf dem Desktop (macOS, Windows, Linux) und im Browser, speichert
jeden Arbeitsbereich als eine Datei, die dir gehört, funktioniert ohne Netz und lässt sich
vollständig von einer KI bedienen.

Gedacht ist plnwrk für Verwaltung und Gefahrenabwehr: Ein **Vorgang** (Aktenzeichen, Frist,
fester Ablauf, z. B. eine Schriftliche Kleine Anfrage) und ein **Projekt** sind dasselbe
Datenmodell.

> **Stand (Oktober 2026): Version 0.2.0** – jung, aber benutzbar.
> **Herunterladen:** [Releases](https://github.com/tobiaswildezahn/plnwrk/releases) (macOS,
> Windows, Linux) · **Im Browser:** [plnwrk.wildezahn.de](https://plnwrk.wildezahn.de) ·
> **Anleitung:** [docs/handbuch.md](docs/handbuch.md). Fahrplan und Entscheidungsprotokoll:
> [docs/PLAN.md](docs/PLAN.md), aktueller Stand: [docs/STATUS.md](docs/STATUS.md).

## Was es kann

- **Projekte und Vorgänge** mit Aufgaben, Unteraufgaben, Meilensteinen, Fristen mit Uhrzeit,
  Zuständigkeiten, Kommentaren und Vorlagen (z. B. „SKA beantworten“).
- **Ansichten:** Übersicht mit Überfälligem, Liste, Kanban-Board (Unteraufgaben in der Karte),
  Meilensteine und **Zeitplan** (Gantt) mit Abhängigkeiten, kritischem Pfad, Warnungen und Zoom.
- **Termine schieben sich mit:** Verschiebt man eine Aufgabe, rutschen die Aufgaben, die auf
  sie warten, so weit wie nötig nach hinten – als ein Rückgängig-Schritt. Jeder Tag zählt, auch
  Wochenende und Feiertag (Gefahrenabwehr arbeitet durch).
- **Nichts geht verloren:** Rückgängig/Wiederholen, lückenloser Verlauf je Aufgabe („wer, wann,
  was“, auch „durch KI“), automatische Wiederherstellungskopien.
- **Eine Datei je Arbeitsbereich** (`.plnwrk`): Änderungsprotokoll mit Hash-Kette, optional mit
  Passphrase verschlüsselt (Pflicht bei VS-NfD).
- **Export:** Zeitplan als Tabelle für Excel; ganze Datei als JSON, CSV oder Markdown.
- **KI:** eingebauter MCP-Server – Claude Code oder Claude Desktop lesen und ändern
  plnwrk-Dateien ([docs/mcp.md](docs/mcp.md)). VS-NfD und verschlüsselte Dateien sind dabei
  gesperrt.
- **Deutsch und Englisch**, hell und dunkel, Befehlspalette (⌘K / Strg+K), bedienbar mit
  Screenreader.

**Verschlusssachen:** plnwrk ist für VS-NfD *ausgelegt*, aber nicht freigegeben – eine Freigabe
kann nur die zuständige Geheimschutzstelle erteilen. Die Browser-Version und der MCP-Server
lehnen VS-NfD grundsätzlich ab.

## Starten

Voraussetzung: [Rust](https://rustup.rs) – die Version legt `rust-toolchain.toml` fest.

```sh
cargo run -p plnwrk                                   # Desktop-App
cargo xtask app && open target/plnwrk.app            # macOS: als App bauen (Doppelklick)
cd apps/plnwrk-web && trunk serve                     # Browser-Version
```

Kommandozeile (Ausgabe immer JSON, auch für Skripte und Agenten):

```sh
cargo run -p plnwrk-cli -- new abteilung.plnwrk --sample      # Datei mit Beispiel anlegen
cargo run -p plnwrk-cli -- query abteilung.plnwrk overdue     # Überfälliges
cargo run -p plnwrk-cli -- commands                           # alle 21 Befehle mit Parametern
cargo run -p plnwrk-cli -- export abteilung.plnwrk --format markdown
```

KI anbinden: [docs/mcp.md](docs/mcp.md) (zwei Befehle für Claude Code).

## Qualität

Eine einzige Schranke prüft alles: Formatierung, Linter (`clippy -D warnings`, nativ und für
WebAssembly), Tests mit Mindestabdeckung, Schichtregeln, Reinheit des Kerns, Lizenzen und
Sicherheitsmeldungen der Abhängigkeiten.

```sh
cargo install --locked cargo-deny cargo-llvm-cov   # einmalig
cargo xtask ci
```

Jede Phase läuft durch ein unabhängiges Review und ein Nachreview; jede Korrektur bekommt einen
Regressionstest, der per Mutationsprobe belegt, dass er den Fehler erkennt. Gebaut wird plnwrk
von Tobias Wildezahn mit Claude (Anthropic) als Entwicklungsagent – das Entscheidungsprotokoll
in [docs/PLAN.md §9](docs/PLAN.md) hält fest, wer was entschieden hat.

## Aufbau

| Schicht | Crate | Aufgabe |
| --- | --- | --- |
| 0 | `plnwrk-core` | Datenmodell, Regeln |
| 1 | `plnwrk-engine` | Befehle, Rückgängig, Verlauf, Abfragen |
| 2 | `plnwrk-store`, `plnwrk-schedule`, `plnwrk-platform` | Dateiformat, Zeitplanung, Uhr und IDs |
| 3 | `plnwrk-mcp` | MCP-Server |
| 4 | `plnwrk-ui` | Oberfläche (egui) |
| Apps | `plnwrk`, `plnwrk-web`, `plnwrk-cli` | Desktop, Browser, Kommandozeile |

Ein Crate darf nur Crates aus tieferen Schichten verwenden; `cargo xtask ci` prüft das. Jede
Änderung an Daten ist ein Befehl – Oberfläche, Kommandozeile und KI benutzen dieselben.

## English

**plnwrk** is project and case management for public administration and emergency services,
written entirely in Rust: desktop and browser, offline, one file per workspace (hash-chained
change log, optional encryption), undo and a full history, a Gantt schedule with dependencies
and critical path, CSV export, German and English UI, and a built-in MCP server so Claude can
read and change workspaces (never classified or encrypted ones). Status: young but usable,
no installer yet. Build with `cargo run -p plnwrk`; quality gate `cargo xtask ci`.

## Lizenz

[Apache-2.0](LICENSE)

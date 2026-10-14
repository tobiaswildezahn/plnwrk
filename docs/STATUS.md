# plnwrk – Stand und offene Punkte

> Übergabedokument für die nächste Session (Mensch oder Agent). Stand: **2026-10-14**.
> Verbindlich bleibt [PLAN.md](PLAN.md); dieses Dokument fasst den Ist-Zustand und alles Offene
> zusammen. Nach jeder Phase aktualisieren.

## 1. Kurzfassung

plnwrk („Planwerk“) ist eine Projekt- und Vorgangsmanagement-Software komplett in Rust, nach
dem Vorbild RustGIS: Desktop + Browser, Datenhoheit (eine Datei je Arbeitsbereich), offline,
voll KI-steuerbar, Fernziel VS-NfD. Projekt und Vorgang sind **ein** Datenmodell.

| Phase | Stand | PR |
| --- | --- | --- |
| 0 Fundament | **fertig**, gemergt | [#1](https://github.com/tobiaswildezahn/plnwrk/pull/1) |
| 1 Kern (Modell, 23 Befehle, 9 Abfragen, Undo/Redo, CLI) | **fertig**, gemergt, 2 Reviews | [#2](https://github.com/tobiaswildezahn/plnwrk/pull/2) |
| 2 Speicherung (`.plnwrk`, Verschlüsselung, Export, CLI auf Dateien) | **fertig**, gemergt, 2 Security-Reviews | [#3](https://github.com/tobiaswildezahn/plnwrk/pull/3) |
| 3a Desktop-MVP (Dateien, Übersicht, Liste, Board, Meilensteine, Details, Vorlagen) | **fertig**, gemergt, Review + Nachreview, von Tobi getestet | [#4](https://github.com/tobiaswildezahn/plnwrk/pull/4) |
| 3b-1 Termine und Meilensteine (Datumsauswahl, Uhrzeiten, Meilenstein = Aufgabe; Format 2) | **fertig**, gemergt, Review + Nachreview | [#5](https://github.com/tobiaswildezahn/plnwrk/pull/5) |
| 3b-2 Nichts verlieren (Autosave, Liste bearbeiten, Schutz aus der App, Barrierefreiheit) | **umgesetzt, PR offen** | Branch `phase-3b-2` |
| 3b-3 Befehlspalette, Englisch, Theme | offen | – |
| 4 Web | offen | – |
| 5 Zeitplanung | offen | – |
| 6 KI (MCP, Assistent) | offen | – |
| 7 VS-Härtung | offen | – |
| 8 Veröffentlichung | offen | – |

## 2. Technischer Ist-Zustand

- **Repo:** `~/projekte/plnwrk/`, GitHub privat `tobiaswildezahn/plnwrk`. `main` enthält
  Phase 0–3b-1 (Merge `28e16d4`). Neue Phase immer auf eigenem Branch `phase-N`.
- **Toolchain:** Rust 1.99.0 (rustup, `rust-toolchain.toml`), Werkzeuge per Homebrew:
  cargo-deny, trunk, cargo-llvm-cov. Neue Shell: `source ~/.cargo/env` falls `cargo` fehlt.
- **Schranke:** `cargo xtask ci` – Format, Schichtregeln, Reinheit core/engine, Clippy nativ +
  wasm32 mit `forbid`-Lints, WASM-Build, Tests, Abdeckung core+engine+store ≥ 90 % (aktuell
  ≈ 95 %), cargo-deny. Letzter lokaler Lauf: grün.
- **CI:** GitHub Actions. Linux, macOS und Web-Bundle bei jedem Push (1–3 min); **Windows
  (≈ 15 min) nur vor dem Merge**, ausgelöst durch das PR-Label `merge` (Entscheidung Tobi
  2026-10-14).
- **Crates:** `core` (Modell, Invarianten, Rang, Policy), `engine` (Befehle, Events, Undo/Redo,
  Abfragen, Fixture), `store` (Dateiformat, Krypto, Export, Dateien), `schedule`/`assistant`/
  `platform` (echte Uhr, Zufalls-IDs), `ui` (egui-Oberfläche, Desktop-MVP),
  `schedule`/`assistant`/`mcp` (Gerüste), Apps `plnwrk` (Desktop mit Systemdialogen),
  `plnwrk-web` (dieselbe Oberfläche im Browser, noch ohne Dateien), `plnwrk-cli`.
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

1. **Tobi testet den MVP** (Demo-Schritt in `docs/plans/2026-10-14-phase-3.md`, Abschnitt
   „Demo“) und meldet, was fehlt oder stört → Rückmeldungen priorisieren 3b.
2. PR `phase-3` nach Review und grüner CI mergen.
3. Phase 3b planen (Befehlspalette, Englisch, Autosave/Wiederherstellung, Inline-Bearbeitung,
   Verschlüsseln aus der App) – Reihenfolge nach Tobis Rückmeldung.
4. Optional: `cargo xtask mutants` (siehe 5.1).

## 4. Offene Entscheidungen des Nutzers

| # | Frage | Vorschlag | Spätestens |
| --- | --- | --- | --- |
| 4.1 | Soll der **Browser VS-NfD-Dateien grundsätzlich nicht öffnen**? | Ja, harte Sperre (Browser gilt laut §3.8 nicht als VS-geeignet; löst auch den scrypt-Speicherkonflikt) | vor Phase 4 |
| 4.2 | **Herabstufung**: Wer darf eine Datei herabstufen, braucht es vier Augen? Herabstufung erzeugt eine neue Datei ohne Geschichte. | Eigener, protokollierter Vorgang mit Begründung; Vier-Augen-Prinzip | vor Phase 7 |
| 4.3 | **Passphrase vergessen = Daten verloren.** Hinterlegung oder zweiter Schlüssel (age kann mehrere Empfänger)? Wer entscheidet in der Organisation? | Zweiter Empfänger (Behördenschlüssel) als Option | vor echtem VS-Einsatz |
| 4.4 | ~~Tempo vs. VS-Niveau in Phase 3~~ **entschieden 2026-10-14:** früh zeigbar (MVP), VS später | – | erledigt |
| 4.5 | **Lizenz**: Apache-2.0 (wie RustGIS) oder EUPL-1.2 (öffentliche Hand, openCode)? | EUPL-1.2 prüfen | vor Phase 8 |
| 4.6 | **KI früher?** MCP-Server (Phase 6) könnte schon auf Dateien aus Phase 2 laufen und plnwrk vor der UI aus Claude bedienbar machen. | Option, ändert Phasenfolge | jederzeit |
| 4.7 | **Identität im Protokoll**: Der Bearbeiter ist frei wählbar, nicht nachgewiesen. Signatur (Dienstausweis, Schlüsseldatei)? | Klären vor Phase 7 | vor Phase 7 |

## 5. Offene technische Punkte

### 5.1 Vorgeschlagen, noch nicht beauftragt
- **`cargo xtask mutants`** (cargo-mutants, nicht blockierend) für `store` und `engine`: macht
  die bisher von Hand geführten Mutationsproben systematisch.
- **Echte Vorgangsvorlagen** der Abteilung (SKA, Beschaffung, Übung) als Testdaten/Vorlagendatei.
- **Eingangskanäle** (Mail, Drucksache/parldok-Import) als spätere Erweiterung – in PLAN.md §9
  als Kandidat vermerken, sobald gewünscht.

### 5.2 Bekannte, bewusst akzeptierte Grenzen (PLAN.md §9)
- **Rollback**: Ersetzen einer verschlüsselten Datei durch eine ältere Fassung mit derselben
  Passphrase wird nicht erkannt (Abhilfe: Signatur/Zähler außerhalb der Datei, Phase 7).
- **Unverschlüsselte Datei** kann bei vollständiger Neuberechnung von Kette und Zustand
  unbemerkt verändert werden (Abhilfe: Signatur, Phase 7).
- „Reproduzierbares Speichern“ gilt nur innerhalb eines Builds (Kompressions-Backend).
- Sperrdateien `.<name>.lock` und nach Absturz ggf. `*.tmp`-Reste bleiben im Ordner liegen.
- Rechte „nur Eigentümer“ (0600) gelten unter Unix; unter Windows keine ACL-Härtung.
- `SaveOptions.work_factor` erlaubt API-Aufrufern niedrigere scrypt-Faktoren (für Tests gedacht).
- Abdeckungszahl enthält In-Datei-Testmodule (Werkzeuggrenze).

### 5.3 Für spätere Phasen vorgemerkt
- **Phase 3b**: Autosave-Auslöser (Pfad-Konvention `.<name>.autosave.plnwrk` steht), i18n
  de/en (Texte stehen zentral in `crates/ui/src/text.rs`), Befehlspalette, Verschlüsseln aus
  der App, Doppelklick auf `.plnwrk`-Dateien im Finder (macOS liefert Dateien per Apple Event,
  nicht als Argument), App-Symbol.
- **Bekannte Grenzen**: Wiederherstellungskopien (Autosave, Beenden über Dock/Abmelden) liegen
  in `~/Library/Application Support/plnwrk/Notkopien` und stehen auf der Startseite; Kopien, die
  man nie öffnet, bleiben dort liegen (Aufräumen von Hand); Cmd+Q ruht, solange ein Dialog offen ist; ohne macOS-Standardmenü
  fehlen Cmd+H/Cmd+M; Dateien in schreibgeschützten Ordnern lassen
  sich nicht öffnen (Sperrdatei); Speichern verschlüsselter Dateien dauert ≈ 1 s und blockiert
  kurz die Oberfläche; Kommentare/Verlauf zeigen Feldnamen technisch (`due`); die Liste ist noch
  nicht direkt editierbar (Details rechts).
- **Phase 4**: Web-App braucht `getrandom` 0.2 `js` / 0.4 `wasm_js` (im store bereits gesetzt);
  Entscheidung 4.1.
- **Phase 7**: Herabstufung (4.2), Signatur (4.7), Export-Verweigerung bei Herabstufung,
  Audit-Bericht.

## 6. Arbeitsweise, die sich bewährt hat

- Phase → Phasenplan → TDD → `cargo xtask ci` grün → PR → **unabhängiger Review-Agent (Opus)**
  → Befunde einarbeiten → **Nachreview** → CI grün auf allen Plattformen → Merge.
- Reviews fanden in **jeder** Phase echte Fehler (Phase 0: Schranke umgehbar; Phase 1:
  Rangraum, quadratische Laufzeit, ungetestete Sicherheitsnetze; Phase 2: VS-Inhalt im Log einer
  entschlüsselbaren Datei). Testlücken (überlebende Mutationen) Phase 2: 14 → 8 → 0.
- Jede Aussage „Test fängt X“ per Mutationsprobe belegen; Skript-Ersetzungen mit
  `assert old in s` absichern (nach `cargo fmt` passen Muster oft nicht – zwei Tests waren sonst
  unbemerkt nicht in der Datei gelandet).
- Windows-only-Probleme zeigt nur die CI (lokal kein Windows-Target).

## 7. Wichtige Dateien

| Datei | Inhalt |
| --- | --- |
| `docs/PLAN.md` | verbindlicher Plan, Regeln, Architektur, Entscheidungsprotokoll §9 |
| `docs/STATUS.md` | dieses Dokument |
| `docs/plans/2026-10-14-phase-{0,1,2}.md` | Aufgabenpläne der Phasen |
| `CLAUDE.md` | Agenten-Einstieg, Befehle, Stolperfallen |
| `xtask/src/{ci,layers,purity}.rs` | die Schranke |
| `crates/store/tests/store.rs` | Angriffs- und Schadenstests des Dateiformats |

# plnwrk mit Claude bedienen (MCP)

plnwrk bringt einen MCP-Server mit. Damit kann Claude Code oder Claude Desktop plnwrk-Dateien
lesen und ändern, etwa so:

- „Was ist in ~/Dokumente/Abteilung.plnwrk überfällig?“
- „Verschiebe alle überfälligen Aufgaben um eine Woche.“
- „Lege im Projekt Digitalfunk eine Aufgabe ‚Funkprobe Wache 12‘ mit Frist 20.11. an.“

## Was Claude darf – und was nicht

- **Jede plnwrk-Datei**, deren Pfad Claude nennt, kann gelesen und geändert werden.
- **Jede Änderung wird sofort gespeichert.** Sie ist im Verlauf als „durch KI“ markiert (in der
  App: Aufgabe → Verlauf).
- **Rückgängig („undo“)** nimmt nur zurück, was Claude selbst getan hat, solange der
  plnwrk-Server läuft. Claude Code startet ihn je Sitzung, Claude Desktop einmal für alle
  Unterhaltungen. Wurde die Datei inzwischen woanders geändert, gibt es nichts mehr rückgängig
  zu machen. Eine Änderung eines Menschen nimmt Claude nie zurück.
- **Termine:** Verschiebt Claude Beginn oder Frist einer Aufgabe, rutschen die Nachfolger mit,
  genau wie in der App (Arbeitstage Hamburg). Nur Termine: ein Rückgängig-Schritt. Termine
  zusammen mit anderen Feldern (z. B. Titel): zwei Schritte, erst die Änderung, dann der
  Schub.
- **Gesperrt:**
  - **VS-NfD:** Eine Datei, die VS-NfD ist (oder es je war), wird nicht geöffnet. Ein Befehl,
    der etwas auf VS-NfD hebt, wird abgelehnt.
  - **Verschlüsselte Dateien** werden nicht geöffnet.
  - **Dateien, die in der App geöffnet sind**, werden nicht geändert. Lesen geht, Claude sieht
    dann den zuletzt gespeicherten Stand.
  - **Schreibgeschützte Dateien** (im Finder „Nur Lesen“) werden nicht geändert.

## Einrichten

1. **Programm installieren** (einmalig, und nach jedem Update erneut):

   ```sh
   cd ~/projekte/plnwrk && cargo install --path apps/plnwrk-cli --locked
   ```

   Danach liegt es unter `~/.cargo/bin/plnwrk-cli`.

2. **In Claude Code anmelden:**

   ```sh
   claude mcp add plnwrk -- ~/.cargo/bin/plnwrk-cli mcp
   ```

   In Claude Code erscheinen dann die Werkzeuge `overview`, `query`, `search_commands` und
   `run_command`. Vor jeder Änderung fragt Claude Code nach, wenn du es nicht anders
   einstellst.

3. **Oder in Claude Desktop:** In der Datei
   `~/Library/Application Support/Claude/claude_desktop_config.json` diesen Eintrag ergänzen
   (`DEINNAME` durch deinen Benutzernamen ersetzen), dann Claude Desktop neu starten:

   ```json
   {
     "mcpServers": {
       "plnwrk": {
         "command": "/Users/DEINNAME/.cargo/bin/plnwrk-cli",
         "args": ["mcp"]
       }
     }
   }
   ```

## Ausprobieren

```sh
plnwrk-cli new ~/Desktop/test.plnwrk --sample
```

Dann in Claude fragen: „Was ist in ~/Desktop/test.plnwrk überfällig? Verschiebe es um eine
Woche.“ Anschließend die Datei in plnwrk öffnen: Die Termine sind verschoben, und im Verlauf
steht „durch KI“.

## Technik (für Entwickler)

- **Protokoll:** MCP über stdio, zeilenweises JSON-RPC 2.0 (`crates/mcp`), ohne `rmcp`.
  Unterstützte Protokollversionen: `2025-06-18`, `2025-03-26`, `2024-11-05`.
- **Werkzeuge:** `overview`, `query` (alle Abfragen der Engine), `search_commands` und
  `run_command` (alle Befehle). Jedes Werkzeug außer `search_commands` nennt seine Datei.
- **Sitzungen:** Der Server hält je Datei die Sitzung, solange die Datei auf der Platte
  unverändert ist, für höchstens 16 Dateien. Darum reichen `undo`/`redo` über mehrere Aufrufe.
- **Eingaben:** Nachrichten bis 4 MiB je Zeile. Gelesen werden nur gewöhnliche Dateien, keine
  Ordner, Pipes oder Geräte.
- **Geprüft wird vor dem Ausführen:**
  - der Kopf der Datei (Einstufung, Verschlüsselung);
  - Inhalt und Verlauf (`classification_of`);
  - die Parameter (`plnwrk_engine::params::asks_for_classified`);
  - vor dem Speichern noch einmal die Einstufung.

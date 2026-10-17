# plnwrk mit Claude bedienen (MCP)

plnwrk bringt einen MCP-Server mit. Damit kann Claude Code oder Claude Desktop plnwrk-Dateien
lesen und ändern, etwa so:

- „Was ist in ~/Dokumente/Abteilung.plnwrk überfällig?“
- „Verschiebe alle überfälligen Aufgaben um eine Woche.“
- „Welche Meilensteine sind überholt, und was hält sie auf?“
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
  genau wie in der App. Jeder Tag zählt, auch Wochenende und Feiertag; eine Woche sind 7 Tage.
  Nur Termine: ein Rückgängig-Schritt. Termine zusammen mit anderen Feldern (z. B. Titel): zwei
  Schritte, erst die Änderung, dann der Schub. Termine, die gleich bleiben, schieben nichts.
- **Meilensteine nur nach Rückfrage:** Ändert ein Aufruf einen Meilenstein (Termin, Titel,
  Status, kein Meilenstein mehr, Löschen), lehnt plnwrk ihn
  ab und nennt, was sich ändern würde. Claude fragt dich dann im Chat und wiederholt den Aufruf
  erst nach deinem Ja mit `"confirmed": true`; die Antwort nennt dann unter
  `milestone_changes`, was sich geändert hat. Einen Meilenstein anlegen fragt nicht nach,
  Rückgängig auch nicht. In der App fragt dieselbe Regel mit einem Dialog nach.
- **Meilensteine rutschen nie mit:** Läge nach neuen Terminen eine Aufgabe hinter einem
  Meilenstein, der auf sie wartet, nennt die Antwort ihn unter `milestones_overtaken`; Claude
  sagt dir das.
- **Verschieben um Tage:** Das Werkzeug `move_task` schiebt eine Aufgabe um N Tage („um eine
  Woche“ = 7), Uhrzeit der Frist bleibt.
- **Antworten** nennen, was sich geändert hat (Aufgabe, neue Termine, mitgerutschte
  Nachfolger); bei „undo“ auch, welcher Schritt zurückgenommen wurde.
- **Gesperrt:**
  - **VS-NfD:** Eine Datei, die VS-NfD ist (oder es je war), wird nicht geöffnet. Ein Befehl,
    der etwas auf VS-NfD hebt, wird abgelehnt.
  - **Verschlüsselte Dateien** werden nicht geöffnet.
  - **Dateien, die in der App geöffnet sind**, werden nicht geändert. Lesen geht, Claude sieht
    dann den zuletzt gespeicherten Stand.
  - **Schreibgeschützte Dateien** (im Finder „Nur Lesen“) werden nicht geändert.

## Einrichten

1. **Programm bereitlegen** – eine der beiden Arten:

   - **Aus dem Paket (ohne Rust):** `plnwrk-cli` liegt im Download neben der App (macOS: im
     .dmg; Windows: `plnwrk-cli.exe` im .zip; Linux: im .tar.gz). An einen festen Ort kopieren,
     z. B. macOS/Linux `~/bin/plnwrk-cli`, Windows `C:\Users\DEINNAME\plnwrk\plnwrk-cli.exe`.
     **macOS** sperrt das heruntergeladene Programm zunächst wie die App: einmal im Terminal
     `~/bin/plnwrk-cli --version` aufrufen, die Meldung schließen und unter
     **Systemeinstellungen → Datenschutz & Sicherheit → Dennoch erlauben** freigeben.
   - **Aus dem Quelltext (mit Rust):**

     ```sh
     cd ~/projekte/plnwrk && cargo install --path apps/plnwrk-cli --locked
     ```

     Danach liegt es unter `~/.cargo/bin/plnwrk-cli`.

   Nach jedem Update erneut bereitlegen. In den folgenden Schritten den eigenen Pfad statt
   `~/.cargo/bin/plnwrk-cli` einsetzen.

2. **In Claude Code anmelden:**

   ```sh
   claude mcp add plnwrk -- ~/.cargo/bin/plnwrk-cli mcp
   ```

   In Claude Code erscheinen dann die Werkzeuge `overview`, `query`, `search_commands`,
   `run_command` und `move_task`. Vor jeder Änderung fragt Claude Code nach, wenn du es nicht anders
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
- **Werkzeuge:** `overview`, `query` (alle Abfragen der Engine, z. B. `overdue` und `due_today`
  für die Lage am Morgen, dazu `overtaken_milestones` aus dem Zeitplan: der Übersicht-Block
  „Überholte Meilensteine“ mit Projekt, Termin, frühestem Tag und dem, was aufhält, der
  nächste zuerst), `search_commands` (sucht auch
  in den Feldbeschreibungen; nennt unter `in_the_app` den Platz im Menüband, z. B.
  „Start › Neu“, damit Claude sagen kann, wo man klickt – Tabelle `plnwrk_engine::places`,
  ein Test hält sie gleich mit dem Menüband), `run_command` (alle Befehle) und `move_task`
  (um N Tage). Jedes Werkzeug außer `search_commands` nennt seine Datei.
- **Meilenstein-Wächter:** `Session::milestone_changes` führt den Befehl auf einer Kopie aus und
  vergleicht (`plnwrk_engine::milestones`); eine Regel für App, MCP und CLI. Ohne
  `confirmed` antwortet der Server mit einem Fehler, der Claude anweist, erst zu fragen;
  `plnwrk-cli run … --confirmed` entspricht dem, ohne das Flag Fehler
  `milestone_not_confirmed` mit der Liste `milestones`.
- **Sitzungen:** Der Server hält je Datei die Sitzung, solange die Datei auf der Platte
  unverändert ist, für höchstens 16 Dateien. Darum reichen `undo`/`redo` über mehrere Aufrufe.
- **Eingaben:** Nachrichten bis 4 MiB je Zeile. Gelesen werden nur gewöhnliche Dateien, keine
  Ordner, Pipes oder Geräte.
- **Geprüft wird vor dem Ausführen:**
  - der Kopf der Datei (Einstufung, Verschlüsselung);
  - Inhalt und Verlauf (`classification_of`);
  - die Parameter (`plnwrk_engine::params::asks_for_classified`);
  - vor dem Speichern noch einmal die Einstufung.

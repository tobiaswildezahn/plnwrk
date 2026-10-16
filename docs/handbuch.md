# plnwrk – Kurzanleitung

plnwrk verwaltet Projekte und Vorgänge – zum Beispiel eine Schriftliche Kleine Anfrage vom
Eingang bis zur Abgabe oder die Umrüstung aller Wachen auf neue Funkgeräte. Alles zu einem
Arbeitsbereich steht in **einer Datei** (`.plnwrk`), die auf deinem Rechner bleibt. plnwrk
braucht kein Internet.

## 1. Installieren

Die Pakete gibt es auf GitHub unter **Releases** (`github.com/tobiaswildezahn/plnwrk/releases`).
Sie sind nicht signiert. Darum warnen macOS und Windows beim ersten Start: Das ist bei kleinen
Projekten ohne kostenpflichtiges Entwicklerzertifikat üblich.

### macOS

1. `plnwrk-…-macos.dmg` herunterladen und doppelklicken.
2. **plnwrk** auf den Ordner **Programme** ziehen.
3. plnwrk in „Programme“ doppelklicken. macOS meldet, die App könne nicht geprüft werden →
   **Fertig**.
4. **Systemeinstellungen → Datenschutz & Sicherheit** öffnen, nach unten scrollen: bei
   „plnwrk wurde blockiert“ auf **Dennoch öffnen** klicken und bestätigen.
5. Ab jetzt startet plnwrk normal per Doppelklick.

Die App läuft auf Apple-Silicon- und Intel-Macs ab macOS 11.

### Windows

1. `plnwrk-…-windows-x86_64.zip` herunterladen, mit Rechtsklick **Alle extrahieren**.
2. `plnwrk.exe` doppelklicken. Windows zeigt „Der Computer wurde durch Windows geschützt“ →
   **Weitere Informationen** → **Trotzdem ausführen**.

### Linux

```sh
tar -xzf plnwrk-*-linux-x86_64.tar.gz
cd plnwrk-*/ && ./plnwrk
```

`plnwrk.desktop` und `plnwrk.png` lassen sich nach `~/.local/share/applications` bzw.
`~/.local/share/icons` kopieren, dann erscheint plnwrk im Startmenü.

### Im Browser

Ohne Installation: die Browser-Version öffnen (Adresse steht im README). Dateien werden dort
hochgeladen und heruntergeladen. **Nicht für Verschlusssachen** – VS-NfD lehnt die
Browser-Version ab.

## 2. Erste Schritte

1. **Beispiel öffnen** auf der Startseite zeigt einen fertigen Arbeitsbereich: ein Projekt
   „Digitalfunk Umrüstung Wachen“ und einen Vorgang „SKA 23/1234 Sirenenwarnung“. Am besten
   erst daran ausprobieren.
2. **Neue Datei** beginnt einen leeren Arbeitsbereich. Links **+ Neu** legt ein Projekt oder
   einen Vorgang an; **+ Aus Vorlage** übernimmt einen festen Ablauf (z. B. „SKA beantworten“)
   mit allen Schritten und Fristen.
3. **Aufgaben:** im Projekt unten „Neue Aufgabe …“ tippen, Enter. Ein Klick auf eine Aufgabe
   öffnet rechts die Details: Status, Zuständig, Beginn, Frist (mit Uhrzeit), Meilenstein,
   Beschreibung, Unteraufgaben, Kommentare und den Verlauf („wer hat wann was geändert“).
4. **Speichern:** ⌘S bzw. Strg+S. Beim ersten Mal fragt plnwrk nach dem Ort. Stürzt etwas ab,
   bietet plnwrk beim nächsten Start eine Wiederherstellungskopie an.

## 3. Ansichten

- **Übersicht:** alle Projekte und Vorgänge mit Fortschritt; darunter alles Überfällige.
- **Liste:** Aufgaben mit Unteraufgaben; Doppelklick auf einen Titel benennt um.
- **Board:** Spalten Offen / In Arbeit / Blockiert / Prüfung / Erledigt. Karten ziehen ändert
  den Status. Unteraufgaben stehen in der Karte und werden dort abgehakt. Zieht man eine Karte
  nach „Erledigt“, während noch Unteraufgaben offen sind, fragt plnwrk, ob sie mit erledigt
  werden sollen.
- **Meilensteine:** die festen Termine (♦) eines Projekts.
- **Zeitplan:** Balken von Beginn bis Frist.
  - **Abhängigkeiten:** in den Details einer Aufgabe unter **Wartet auf** einen Vorgänger
    wählen; im Zeitplan erscheint ein Pfeil.
  - **Verschieben:** einen Balken ziehen. Was darauf wartet, rutscht so weit wie nötig mit
    (jeder Tag zählt, auch Wochenende und Feiertag). Rückgängig (⌘Z / Strg+Z) nimmt alles in
    einem Schritt zurück.
  - **Rot** ist der kritische Pfad: Verzögert sich eine dieser Aufgaben, verschiebt sich das
    Ende. Warnungen unter dem Zeitplan nennen, was nicht hält.
  - **Zoom:** − und + oder Strg/⌘ + Mausrad; grob zeigt Monate.
  - **Als Tabelle exportieren (Excel):** speichert den Zeitplan als Datei, die Excel direkt
    öffnet.

## 4. Nützlich zu wissen

- **Befehlspalette:** ⌘K / Strg+K – tippen, was man tun will („Zeitplan“, „Erledigt“,
  „Gehe zu …“).
- **Sprache und Darstellung:** Menü **Ansicht** (Deutsch/English, hell/dunkel).
- **Schützen:** Menü **Datei → Mit Passphrase schützen** verschlüsselt die Datei. Ohne
  Passphrase ist der Inhalt dann nicht lesbar – sie lässt sich nicht wiederherstellen.
- **Verschlusssachen:** plnwrk kann Inhalte als VS-NfD einstufen und verlangt dann
  Verschlüsselung. Eine **Freigabe für VS-NfD hat plnwrk nicht** – darüber entscheidet die
  zuständige Geheimschutzstelle.
- **KI:** Claude kann plnwrk-Dateien lesen und ändern – Anleitung in [mcp.md](mcp.md). Das
  Programm dafür (`plnwrk-cli`) liegt in jedem Paket neben der App. VS-NfD und verschlüsselte
  Dateien bekommt die KI nie zu sehen.

## 5. Fragen und Fehler

Auf GitHub unter **Issues** melden – mit kurzer Beschreibung, was du getan hast, was passieren
sollte und was passiert ist. Bitte nie echte Dienstdaten oder Verschlusssachen anhängen.

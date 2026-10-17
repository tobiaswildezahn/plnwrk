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

Auf **Dienst- und Behördenrechnern** entscheidet meist die IT, welche Programme starten dürfen;
dort fehlt der Knopf „Trotzdem ausführen“ oft ganz. Dann bitte die IT fragen – nicht umgehen.

### Linux

```sh
tar -xzf plnwrk-*-linux-x86_64.tar.gz
cd plnwrk-*/ && ./plnwrk
```

Für das Startmenü einmalig:

```sh
mkdir -p ~/.local/bin ~/.local/share/applications ~/.local/share/icons
cp plnwrk plnwrk-cli ~/.local/bin/
cp plnwrk.desktop ~/.local/share/applications/
cp plnwrk.png ~/.local/share/icons/
```

Läuft auf Ubuntu ab 22.04, Debian ab 12 und vergleichbar aktuellen Systemen.

### Im Browser

Ohne Installation: die Browser-Version unter https://plnwrk.wildezahn.de öffnen. Dateien werden dort hochgeladen und heruntergeladen. **Nicht für
Verschlusssachen** – VS-NfD lehnt die Browser-Version ab.

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
  - **Zoom:** Lupen-Knöpfe, Reiter **Zeitplan**, Strg/⌘ + Mausrad oder mit zwei Fingern auf dem
    Trackpad – überall im Zeitplan; der Tag unter dem Zeiger bleibt stehen.
  - **Mausrad:** bewegt die Zeilen, wenn es mehr gibt als Platz, sonst die Zeit;
    Shift + Mausrad bewegt immer die Zeit.
  - **Als Tabelle exportieren (Excel):** speichert den Zeitplan als Datei, die Excel direkt
    öffnet.

## 4. Nützlich zu wissen

- **Menüband (Ribbon):** Oben stehen die Reiter **Datei**, **Start** und **Ansicht**, darunter die
  Befehle in Gruppen. Ist eine Aufgabe gewählt, kommt der Reiter **Aufgabe** dazu (Status,
  Löschen); im Zeitplan der Reiter **Zeitplan-Werkzeuge** (Zoom, Export). Doppelklick auf
  einen Reiter, der Pfeil oben rechts oder ⌘F1 / Strg+F1 (am Mac auch ⌘⌥R) klappt das Menüband
  ein und aus. Eingeklappt öffnet ein Klick auf einen Reiter es kurz über dem Inhalt; ein
  Klick daneben oder Esc schließt es wieder. Nur in sehr niedrigen Fenstern startet es
  eingeklappt. Rückgängig und Wiederholen stehen immer oben rechts.
- **Befehlspalette:** ⌘K / Strg+K – tippen, was man tun will („Zeitplan“, „Erledigt“,
  „Gehe zu …“). Jeder Treffer nennt seinen Platz im Menüband, z. B. „Start › Neu“.
- **Sprache und Darstellung:** Reiter **Ansicht** (Deutsch/English, hell/dunkel).
- **Schützen:** Reiter **Datei → Mit Passphrase schützen** verschlüsselt die Datei. Ohne
  Passphrase ist der Inhalt dann nicht lesbar – sie lässt sich nicht wiederherstellen.
- **Verschlusssachen:** **Version 0.1.0 ist nicht für echte Verschlusssachen gedacht.**
  plnwrk kann Inhalte als VS-NfD kennzeichnen und verlangt dann Verschlüsselung, aber eine
  Freigabe hat es nicht – darüber entscheidet allein die zuständige Geheimschutzstelle, und
  die dafür nötige Härtung ist noch nicht gebaut.
- **KI:** Claude kann plnwrk-Dateien lesen und ändern – Anleitung in [mcp.md](mcp.md). Das
  Programm dafür (`plnwrk-cli`) liegt in jedem Paket neben der App. Über diese Schnittstelle
  bekommt die KI VS-NfD-Dateien und verschlüsselte Dateien nie zu sehen.

## 5. Fragen und Fehler

Auf GitHub unter **Issues** melden – mit kurzer Beschreibung, was du getan hast, was passieren
sollte und was passiert ist. Bitte nie echte Dienstdaten oder Verschlusssachen anhängen.

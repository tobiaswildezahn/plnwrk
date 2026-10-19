# Phase 10a – Gemeinsame Datei (mehrere Bearbeiter)

> Auftrag Tobi (2026-10-19): „Ich möchte, dass es leichtgewichtig für mehrere Nutzer nutzbar wird.
> Dazu halte ich das Arbeiten mit einer gemeinsamen Datei am sinnvollsten. Setze das zielgerichtet
> um. Ich muss dafür natürlich Nutzer/Sachbearbeiter anlegen und natürlich auch ändern können.“
>
> Teil von Phase 10 (Team). 10a kommt ohne Server aus; ein Server (PLAN §7, `server`-Crate)
> bleibt späteren Teilen vorbehalten und setzt auf 10a auf.

## 1. Ziel

Mehrere Personen arbeiten an **derselben `.plnwrk`-Datei**, typischerweise auf einem
Abteilungslaufwerk. Dabei gilt:

- Jede Person öffnet die Datei, wann sie will, auch gleichzeitig mit anderen.
- Beim **Speichern** führt plnwrk die eigenen Änderungen mit denen zusammen, die andere
  inzwischen gespeichert haben. Niemandes Arbeit geht still verloren.
- Wer nichts Ungespeichertes hat, sieht die Änderungen der anderen nach wenigen Sekunden von
  selbst.
- Jede Änderung ist einer Person zugeordnet. Personen (Sachbearbeiter) lassen sich anlegen und
  **ändern**.

Erfolgsmaß: Zwei Personen bearbeiten dieselbe Datei abwechselnd und gleichzeitig. Danach
stehen die Änderungen beider in der Datei, die Arbeit keiner Person ging still verloren, und
der Verlauf zeigt, wer was getan hat.

## 2. Entscheidungen

**Tobi (2026-10-19):**
- Leichtgewichtig, über eine gemeinsame Datei, **kein Server**.
- Personen anlegen und ändern.

**Des Agenten (Abweichungen bitte im Review ansprechen):**

1. **Sperre nur beim Speichern.** Heute sperrt plnwrk die Datei, solange sie offen ist; eine
   zweite Person kann sie dann gar nicht öffnen. Künftig hält plnwrk die Sperre nur für den
   Moment des Speicherns: lesen, zusammenführen, schreiben. CLI und MCP arbeiten schon so.
2. **Zusammenführen Feld für Feld.** Die eigenen Änderungen seit dem letzten Abgleich werden
   auf den Stand der Datei gesetzt (PLAN §3.4).
   - Ändern beide dasselbe Feld, gilt der Wert dessen, der **später speichert**.
   - Ist der Konflikt in einer Aufgabe, bekommt sie einen **Kommentar**. So sieht ihn auch die
     andere Person, deren Wert ersetzt wurde.
   - Wer speichert, sieht eine Meldung.
3. **Löschen vor Ändern.** Hat jemand eine Aufgabe gelöscht, die ich geändert habe, bleibt sie
   gelöscht. Meine Änderung geht verloren und wird in der Meldung genannt.
4. **Regeln gehen vor.** Ergibt das Zusammenführen etwas Unzulässiges, wird die betroffene
   eigene Änderung nicht übernommen und genannt. Beispiele: ein Kreis aus Abhängigkeiten, eine
   Unteraufgabe unter einer gelöschten Aufgabe.
5. **Rückgängig** gilt weiter für die eigenen Schritte, auch nach einem Abgleich. Änderungen
   anderer lassen sich nicht rückgängig machen; man ändert sie wie jede andere Angabe.
6. **Wer bin ich?** plnwrk merkt sich je Gerät und Datei, als welche Person man arbeitet.
   Eine Datei mit Änderungen mehrerer Personen gilt als gemeinsam. Ist dort noch niemand
   gewählt, fragt plnwrk beim Öffnen einmal danach.
7. **Personen ändern.** Die Ansicht „Personen“ bekommt „Bearbeiten“ für Name, Kürzel, Rolle
   und E-Mail. Der Befehl `person.update` gibt es schon. **Kein Löschen:** Der Verlauf verweist
   auf die Person.
8. **Kein neues Dateiformat.** 10a fügt dem Modell kein Feld hinzu. Dateien bleiben Version 2;
   0.2.0 und 0.3.0 lesen sich gegenseitig.

## 3. So sieht es für die Bearbeiter aus

1. Tobi legt die Datei auf dem Laufwerk an und legt die Personen an: „Personen“ →
   „+ Person“. Später ändert er sie dort über „Bearbeiten“.
2. Anna öffnet die Datei. plnwrk fragt einmal: „Wer bist du?“ Anna wählt sich aus. Das gilt
   auf ihrem Rechner für diese Datei, bis sie es ändert.
3. Anna und Ben arbeiten gleichzeitig. Ben speichert zuerst. Anna hat nichts Ungespeichertes
   und sieht nach wenigen Sekunden: „Aktualisiert: 2 Änderungen von Ben Kruse.“
4. Beide ändern die Frist derselben Aufgabe. Ben speichert, dann Anna. Anna sieht:
   „Gespeichert und zusammengeführt: 1 Änderung von Ben Kruse übernommen; bei 1 Angabe galt
   deine (Frist von „Einbau“, Ben hatte 20.11.2026).“ Die Aufgabe bekommt einen Kommentar.
5. Speichert gerade jemand anderes, wartet plnwrk kurz. Bleibt die Datei gesperrt, sagt es
   das; die Änderungen bleiben ungespeichert und in der Wiederherstellungskopie erhalten.

## 4. Technik

### 4.1 Zusammenführen (Engine, rein)

Neues Modul `crates/engine/src/sync.rs`:

```text
rebase(file: Document-Teile (Workspace, Log), base: Länge des gemeinsamen Anfangs,
       own: eigene Änderungen seit base) -> Ergebnis { workspace, log, Zuordnung alt→neu,
       übernommene fremde Änderungen, Konflikte, nicht übernommene eigene Änderungen }
```

- Ob die Datei auf `base` aufbaut, zeigt der Hash der Zeile `base` in der Kette. Stimmt er
  nicht (Datei ersetzt, „Speichern unter“ einer anderen Datei), wird nicht zusammengeführt;
  das Speichern fragt nach einem anderen Ort.
- Jede eigene Änderung wird mit neuer laufender Nummer angehängt. Zeit, Person, Herkunft,
  Befehl und Parameter bleiben gleich, die Ereignisse werden neu berechnet:
  - **`Put` mit `before`**: Feld für Feld (JSON, ohne `entity`). Felder, die ich geändert
    habe, nehmen meinen Wert. Hat die Datei dort einen dritten Wert, ist das ein Konflikt.
  - **`Put` ohne `before`** (neu angelegt): unverändert.
  - **`Remove`**: entfernt den aktuellen Stand. Ist er schon weg, entfällt die Änderung.
  - **Entität weg, ich habe geändert**: Die Änderung entfällt und wird genannt.
- Nach jeder Änderung `Workspace::check()`. Bei Verstoß wird die Änderung zurückgerollt und
  genannt.
- `undoes`/`redoes` werden auf die neuen Nummern umgeschrieben.
- Rückgängig-Stapel: eigene Nummern alt → neu. Entfallene Änderungen fallen heraus.
- Konflikte in Aufgaben: danach je Aufgabe ein `comment.add`, Herkunft Skript, mit beiden
  Werten.

### 4.2 Speichern und Sperre (App)

- `OpenDoc` hält keine Sperre mehr, solange die Datei offen ist. Es merkt sich den
  **Abgleichstand**: Länge und Kopf-Hash des Logs beim Öffnen bzw. letzten Speichern sowie
  Größe und Zeitstempel der Datei.
- `save`:
  1. Sperre nehmen; bis etwa 3 s erneut versuchen.
  2. Datei lesen.
  3. Kopf gleich Abgleichstand: wie bisher schreiben.
  4. Sonst `rebase`, die Sitzung durch das Ergebnis ersetzen (Auswahl und Ansicht bleiben),
     schreiben.
  5. Sperre lösen; Meldung, wenn etwas zusammengeführt wurde.
- „Speichern unter“ an einen neuen Ort wie bisher. Nur „Speichern“ führt zusammen.
- Die Wiederherstellungskopien bleiben lokal und unverändert.

### 4.3 Abholen

- Alle 3 s prüft die App Größe und Zeitstempel der Datei, nur auf dem Desktop.
- Hat sich die Datei geändert:
  - Ist nichts ungespeichert, liest plnwrk sie ohne Sperre neu ein (`rebase` ohne eigene
    Änderungen). Meldung: „Aktualisiert: n Änderungen von …“.
  - Ist etwas ungespeichert, zeigt die Statuszeile „Neue Änderungen in der Datei – werden beim
    Speichern zusammengeführt“.
- Ist die Datei verschlüsselt, braucht das Neueinlesen die Passphrase der Sitzung. Sie liegt
  wie heute nur im Speicher.

### 4.4 Personen und „Ich arbeite als“

- `views/people.rs`: Liste mit „Bearbeiten“ je Person. Das Formular ist dasselbe wie beim
  Anlegen und führt `person.update` aus; ungespeichert getippte Werte zählen wie jede Eingabe
  (CLAUDE.md, `flush_draft`).
- Gemerkt wird in den App-Einstellungen (eframe-Speicher): `kanonischer Pfad → Person-ID`.
- Gemeinsam = im Log stehen Änderungen von mindestens zwei Personen. Ist dort niemand gewählt,
  öffnet beim Öffnen der Dialog „Wer bist du?“ mit den Personen und „Person anlegen…“.
- Die Statuszeile zeigt die gewählte Person; ein Klick ändert sie.

### 4.5 CLI, MCP, ältere Versionen

- CLI und MCP: unverändert (sperren, lesen, ändern, schreiben). Neu ist nur, dass sie auch
  schreiben dürfen, während die App die Datei offen hat. Die App holt das ab.
- Eine **ältere plnwrk-Version** (0.2.0) hält die Sperre, solange sie die Datei offen hat. Dann
  kann 0.3.0 nicht speichern und sagt das. Es geht nichts verloren, aber alle sollten dieselbe
  Version nutzen (Handbuch).

## 5. Nicht in 10a

- Echtzeit und ein Server; Rechte und Rollen (wer darf was sehen); Benachrichtigungen.
- Nachgewiesene Identität (STATUS 4.7): Die gewählte Person ist eine Angabe, kein Nachweis.
  Das Handbuch sagt das.
- Browser-Version: Sie arbeitet mit Hochladen und Herunterladen, eine gemeinsame Datei gibt es
  dort nicht.
- Verschlusssachen: Eine gemeinsame Datei teilt auch die Passphrase. Die Weitergabe regelt die
  Abteilung (Geheimschutzstelle).
- Personen löschen oder deaktivieren.

## 6. Schritte

| Teil | Inhalt | Belegt durch |
| --- | --- | --- |
| **10a-1** | `sync::rebase` in der Engine | Einheitstests je Regel (Feld, Konflikt, gelöscht, Regelverstoß, Rückgängig) und ein Eigenschaftstest: zwei zufällige Folgen von Befehlen auf gemeinsamem Anfang → Ergebnis prüfbar wie eine echte Datei, alle fremden Änderungen enthalten |
| **10a-2** | Speichern mit kurzer Sperre und Zusammenführen; Meldungen; Kommentare bei Konflikten | UI-Tests mit **zwei App-Instanzen** auf derselben Datei; gesperrte Datei; ersetzte Datei |
| **10a-3** | Abholen ohne Ungespeichertes; Hinweis mit Ungespeichertem | UI-Tests mit zwei Instanzen; Zeitstempel |
| **10a-4** | Personen bearbeiten; „Ich arbeite als“ gemerkt und abgefragt | UI-Tests, auch nur mit Tastatur |
| **10a-5** | Durchgang von Hand mit zwei App-Fenstern auf einer Datei; Handbuch; Version 0.3.0 vorbereitet | Bilder in `docs/ux/` |

Jeder Teil mit Gegenprobe (CLAUDE.md). Am Ende Review und Nachreview durch unabhängige Agenten,
dann PR. Merge auf Tobis „ja, merge“.

## 7. Review-Fokus (vorab)

- Zwei speichern fast gleichzeitig. Die Sperre muss das Lesen, Zusammenführen und Schreiben
  umschließen, sonst überschreibt einer den anderen.
- Eigene Änderung an einer Aufgabe, die der andere gelöscht hat. Eigene Unteraufgabe unter
  einer gelöschten Aufgabe. Abhängigkeiten in beide Richtungen, also ein Kreis.
- Rückgängig nach dem Zusammenführen: die eigene Änderung, nicht die fremde. Rückgängig einer
  eigenen Änderung, die inzwischen jemand anders weiter geändert hat: Das wird abgelehnt und
  gesagt.
- Meilensteine: Die Rückfrage gilt für eigene Änderungen beim Machen, nicht beim
  Zusammenführen. Eine fremde Meilenstein-Änderung kommt ohne Rückfrage an, aber sichtbar.
- Ein getipptes, noch nicht übernommenes Feld beim Abholen: Es darf nicht verloren gehen,
  deshalb holt plnwrk nur ab, wenn nichts ungespeichert ist.
- Verschlüsselte gemeinsame Datei; Datei durch eine andere ersetzt; Datei gelöscht oder
  umbenannt, während sie offen ist.
- Sperren auf einem Netzlaufwerk (SMB). Lokal geprüft; auf dem echten Laufwerk ist es Teil des
  Durchgangs von Hand.

## Demo für Tobi (nach 10a)

1. plnwrk 0.3.0 zweimal öffnen (zwei Fenster, eins je „Person“) oder auf zwei Rechnern mit
   derselben Datei auf dem Laufwerk.
2. In beiden „Ich arbeite als“ wählen; plnwrk fragt bei der gemeinsamen Datei selbst.
3. In Fenster A eine Aufgabe anlegen und speichern; Fenster B zeigt sie nach wenigen Sekunden.
4. In beiden die Frist derselben Aufgabe ändern, nacheinander speichern; die Meldung und den
   Kommentar ansehen.
5. „Personen“ → „Bearbeiten“: einen Namen ändern.

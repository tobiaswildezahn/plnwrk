# Dateien im Format Version 1

Mit plnwrk aus Phase 3a (Commit `3e82c79`) erzeugt, **nie von Hand ändern**:

- `v1-sample.plnwrk` – Beispiel-Arbeitsbereich, unverschlüsselt (`plnwrk-cli new … --sample`)
- `v1-sample-encrypted.plnwrk` – dasselbe verschlüsselt, Passphrase `v1-fixture-passphrase`

Sie beweisen, dass neuere Versionen alte Dateien lesen und umwandeln können
(`crates/store/tests/store.rs`, Abschnitt „Format Version 1“).

# Befehle von plnwrk 0.2.0

`commands-0.2.0.json` – die Parameter-Schemata aller Befehle, wie `plnwrk-cli commands` von
plnwrk 0.2.0 sie ausgibt (Befehl → `input_schema`), **nie von Hand ändern**. plnwrk 0.2.0
prüft beim Öffnen jede Änderung gegen diese Schemata; weicht eines ab, kann 0.2.0 neue Dateien
nicht mehr lesen (Wächtertest `the_commands_stay_those_of_0_2_0`, Phase 10a).

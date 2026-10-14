# Dateien im Format Version 1

Mit plnwrk aus Phase 3a (Commit `3e82c79`) erzeugt, **nie von Hand ändern**:

- `v1-sample.plnwrk` – Beispiel-Arbeitsbereich, unverschlüsselt (`plnwrk-cli new … --sample`)
- `v1-sample-encrypted.plnwrk` – dasselbe verschlüsselt, Passphrase `v1-fixture-passphrase`

Sie beweisen, dass neuere Versionen alte Dateien lesen und umwandeln können
(`crates/store/tests/store.rs`, Abschnitt „Format Version 1“).

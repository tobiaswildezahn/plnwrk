# Schrift: Fira Sans

- **Herkunft:** Fira Sans 4.3 (Mozilla/Carrois, Telefónica), Schnitte Regular und SemiBold,
  aus TeX Live 2025 (`texmf-dist/fonts/opentype/public/fira/`).
- **Lizenz:** SIL Open Font License 1.1, Text in [OFL.txt](OFL.txt). Kein „Reserved Font
  Name“; eine Teilmenge darf darum den Namen behalten.
- **Bearbeitung (2026-10-17, Phase 9a)** mit fontTools:
  - Teilmenge: U+0020–024F (Latein), U+2000–206F (Satzzeichen), U+20A0–20CF (Währung),
    U+2100–214F, U+2190–21FF (Pfeile), U+2200–22FF, U+25A0–25FF; Layout-Merkmale `kern`,
    `liga`, `locl`, `ccmp`, `mark`, `mkmk`. Etwa 95 KB statt 500 KB je Schnitt.
  - **Tabellenziffern als Standard:** Die Ziffern 0–9 zeigen auf die Glyphen des Merkmals
    `tnum`. egui kann OpenType-Merkmale nicht einschalten, Termine und Zahlen sollen aber
    untereinander stehen.
- Was Fira nicht hat (♦, Emoji), kommt aus den eingebauten Schriften von egui dahinter
  (`crates/ui/src/design/mod.rs`, `fonts`).

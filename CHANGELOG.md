# Changelog

## 0.1.0 — 2026-10-22 · « Premier lecteur de kits »

- **Instrument VST3 et CLAP** (nih-plug, Rust) qui joue les kits de 20 percussions produits par [make it drum](https://github.com/yrbane/makeItDrum) depuis le MIDI : pad `i` → note General MIDI `36 + i` (BOOM = 36 … CHAOS = 55).
- **Sources de kits** : le `makeitdrum_pack.zip` exporté par la page (tel quel), un `.kit.zip` avec `kit.json` (schéma v1 : nom, recettes, notes par pad), ou un dossier de WAV.
- **20 voix one-shot**, polyphonie entre pads, redéclenchement dur, gain par vélocité, lecture au ratio d'échantillonnage par interpolation linéaire (WAV 16/24/32 bits ou flottant, mono ou stéréo).
- **Selectbox de kits** : les `.zip` de `~/Documents/makeitdrum/kits/` (ou `MAKEITDRUM_KITS`), chargement à la volée sur un thread de fond, champ chemin + « Charger », glisser-déposer d'un fichier.
- **État persistant** : chemin et octets du kit embarqués dans le preset, le projet se rouvre sans le fichier.
- **Interface egui** : crédit à Masao « Saiya Geez », version, gain, grille 5 × 4 de pads allumés pendant la lecture et cliquables.
- **Qualité** : cœur pur testé (43 tests, golden audio), thread audio sans allocation ni verrou bloquant, CI fmt + clippy + tests + bundles Linux, Windows et macOS, release automatique sur tag.

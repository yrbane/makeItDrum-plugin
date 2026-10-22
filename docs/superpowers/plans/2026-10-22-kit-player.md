# Kit player — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans.

**Goal:** un plugin VST3/CLAP nih-plug qui joue les kits make it drum depuis le MIDI, avec selectbox de kits, état persistant et UI egui.

**Architecture:** crate `core` pure (kit, drummap, voice, engine, wav) testée en `cargo test` ; crate `plugin` mince (params, slot partagé, triggers, éditeur, Plugin). Voir la spec.

**Tech Stack:** Rust 1.88+, nih-plug (git master), nih_plug_egui, hound, zip, serde_json, base64, dirs.

**Spec:** `docs/superpowers/specs/2026-10-22-kit-player-design.md`

## Global Constraints
- Code en anglais, commentaires et commits en français, aucune mention d'assistant.
- Thread audio sans allocation ni verrou bloquant.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` verts à chaque commit.

## Review Focus
1. ZIP avec plus ou moins de 20 WAV → les pads manquants restent muets, pas de panique.
2. WAV à 48 kHz joué par un hôte à 44,1 kHz → ratio > 1, fin de lecture correcte.
3. Note-on de vélocité 0 → traité comme note-off (ignoré, pas de déclenchement).
4. Preset avec octets base64 corrompus → erreur affichée, plugin muet mais vivant.
5. Dossier de kits absent → selectbox vide, pas d'erreur bloquante.

## Tâches
1. `core::wav` : décodage hound vers `Sample` (16/24/32 bits int, f32, mono/stéréo).
2. `core::kit` : `Kit::from_zip_bytes`, `from_dir`, `from_path` ; `kit.json` v1 ; pack sans manifeste ; erreurs.
3. `core::drummap` : `pad_for_note(kit, note)`.
4. `core::voice` : one-shot interpolé, vélocité, ratio, mono/stéréo, fin.
5. `core::engine` : 20 voix, `note_on`, `render` additif, `set_kit`, `playing_mask` ; golden.
6. `plugin` : Params (gain, persist), KitSlot, triggers, Plugin sans UI ; test de round-trip d'état.
7. `plugin::editor` : egui (selectbox, chemin, gain, pads, crédit, version) ; scan du dossier de kits.
8. CI, release, README, CHANGELOG, bundle local, cross-build Windows.

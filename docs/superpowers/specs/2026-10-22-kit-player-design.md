# make it drum — kit player (VST3 / CLAP) — conception

Date : 2026-10-22. Dépôt : `yrbane/makeItDrum-plugin`. Réponse à la phase 4a de
`makeItDrum/docs/proposals/2026-10-22-evolutions-vst-midi-ableton.md`.

## Intention

Un instrument VST3/CLAP qui **joue** les kits produits par la page make it drum
(20 percussions WAV) depuis des notes MIDI, pour qu'un kit fabriqué sur le web
devienne jouable dans Live (chez Seb : Live 12 sous Wine, donc build Windows)
et dans tout DAW. Le son est strictement celui de la page : ce sont les mêmes
fichiers. Le plugin ne génère rien.

## Objectifs / non-objectifs

- Charger un kit depuis : le `makeitdrum_pack.zip` exporté aujourd'hui par la
  page (20 × `makeitdrum_NN.wav`, ZIP « store ») ; un `.kit.zip` contenant
  `kit.json` (schéma v1 ci-dessous) ; un dossier de WAV (tri par nom, 20 max).
- Mapping General MIDI : pad `i` → note `36 + i` (BOOM = 36 … CHAOS = 55) ;
  `kit.json` peut surcharger `note` par pad. Les autres notes sont ignorées.
- 20 voix one-shot, une par pad, polyphonie entre pads, **redéclenchement dur**
  d'un même pad, gain = vélocité (0..1, linéaire), note-off ignoré.
- Lecture au ratio `kit_sr / host_sr` par interpolation linéaire (pas de
  rééchantillonnage en mémoire). Mono → dupliqué sur L/R ; stéréo → L/R.
- Paramètre unique `gain` maître (−30 dB … +6 dB, 0 dB par défaut, lissé).
- **Selectbox de kits** : scan non récursif de `~/Documents/makeitdrum/kits/`
  (`dirs::document_dir()`, surchargeable par `MAKEITDRUM_KITS`), fichiers
  `*.zip` ; choisir = charger immédiatement, sans clic supplémentaire.
  Chargement sur un thread de fond (`BackgroundTask`), jamais sur l'audio.
- Champ « chemin » + bouton « Charger » ; glisser-déposer d'un `.zip` sur la
  fenêtre si l'hôte le transmet (meilleur effort).
- État persistant dans le preset : chemin du kit **et** octets du ZIP (base64)
  pour qu'un projet se rouvre sans le fichier.
- Interface egui : titre « make it drum — kit player vX.Y.Z », crédit
  « un projet de Masao « Saiya Geez » » avec lien beacons, selectbox, chemin,
  gain, grille 5 × 4 de pads (nom de recette ou NN, numéro de note), pad
  allumé pendant sa lecture, clic sur un pad = déclenchement à vélocité 1.
- Non-objectifs : génération dans le DAW, sorties multiples, accordage par pad,
  séquenceur, enveloppes, mode choke.

## Format `kit.json` (schéma v1, partagé avec le web)

```json
{ "schemaVersion": 1, "app": "make-it-drum", "name": "Casserole bleue",
  "pads": [ { "index": 0, "flavor": "BOOM", "note": 36, "file": "01_BOOM.wav" } ] }
```
Champs ignorés s'ils sont absents : `note` (→ 36 + index), `flavor` (→ NN),
`name` (→ nom du fichier ZIP sans extension). Un `kit.json` invalide ou un
`pads[].file` absent du ZIP = erreur de chargement, kit précédent conservé.

## Architecture

Workspace Cargo, deux crates + xtask :

- `crates/core` (`make_it_drum_core`, **pur**, testé sans hôte) :
  `kit` (lecture ZIP/dossier/octets, validation, `Kit { name, pads: Vec<Pad { label, note, sample: Sample { data: Vec<f32> interleaved, channels, sample_rate } }> }`),
  `drummap` (note → index de pad), `voice` (lecture one-shot interpolée),
  `engine` (`Engine { voices: [Voice; 20] }`, `note_on(pad, velocity)`,
  `render(out_l, out_r)` additif, `set_kit(Arc<Kit>)`, `playing_mask()`),
  `wav` (décodage hound 16/24/32 bits et float, 1–2 canaux).
- `crates/plugin` (`make_it_drum`, nih-plug) : `Params` (gain + persist),
  `KitSlot` (partage GUI/fond/audio : `Mutex<Option<Arc<Kit>>>` + compteur
  de génération ; l'audio clone l'Arc quand la génération change ; les 4
  derniers kits sont gardés vivants côté fond pour que l'audio ne libère jamais),
  `triggers` (`AtomicU32` masque de pads à déclencher depuis la GUI),
  `editor` (egui), `lib` (le `Plugin`).
- Thread audio : zéro allocation (`assert_process_allocs` en debug), pas de
  verrou bloquant (`try_lock`).

## Plateformes et livraison

- Linux x86_64 (`.vst3`, `.clap`), Windows x86_64 (`.vst3`, `.clap`, cible
  `msvc` en CI, `gnu` en local via mingw pour le Live sous Wine de Seb),
  macOS universel si le runner est disponible.
- CI : fmt, clippy `-D warnings`, tests, `cargo xtask bundle` par OS, artefacts.
  Release sur tag `v*` : zips par plateforme joints.
- Licence MIT, auteur Masao « Saiya Geez », contributeur yrbane. Version
  SemVer dans `Cargo.toml` (workspace), affichée dans l'UI.

## Vérification

- Tests unitaires du cœur (kit, drummap, voice, engine, wav).
- Golden audio : kit synthétique (20 WAV générés en test) + séquence de notes
  → buffer stéréo comparé à `tests/golden/engine_sequence.bin` (tolérance 1e-6).
- Smoke : le bundle se charge dans un hôte (vérification manuelle par Seb
  dans Live ; en CI, `cargo xtask bundle` suffit).

## Laissé au constructeur

Couleurs/typo de l'UI (sobre, fond sombre, accent vert XP), disposition
exacte, taille de fenêtre (≈ 560 × 420), texte des messages d'erreur.

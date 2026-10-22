# make it drum — kit player 🥁

> un projet de [Masao « Saiya Geez »](https://beacons.ai/saiyageez) · plugin par yrbane

Un instrument **VST3 / CLAP** qui joue, depuis le MIDI, les kits de 20 percussions
fabriqués sur la page [make it drum](https://yrbane.github.io/makeItDrum/)
([dépôt](https://github.com/yrbane/makeItDrum)). Le son est strictement celui de la
page : ce sont les mêmes fichiers WAV. Le plugin ne génère rien, il joue.

- **Version** : 0.1.0 — voir [CHANGELOG.md](CHANGELOG.md)
- **Télécharger** : [releases](https://github.com/yrbane/makeItDrum-plugin/releases) (Linux, Windows, macOS)

## Utilisation

1. Sur la page make it drum, importez un son, puis **Exporter le pack** : vous obtenez
   `makeitdrum_pack.zip`.
2. Déposez ce fichier dans `~/Documents/makeitdrum/kits/` (ou dans le dossier donné
   par la variable `MAKEITDRUM_KITS`).
3. Dans votre DAW, ajoutez l'instrument **make it drum**, choisissez le kit dans la
   selectbox (ou collez un chemin, ou déposez le `.zip` sur la fenêtre).
4. Jouez : pad 1 = note 36 (C1), pad 2 = 37, … pad 20 = 55. Un clic sur un pad le
   déclenche aussi. Le kit est embarqué dans le preset : le projet se rouvre sans le
   fichier.

| Recette | BOOM | KICK | THUD | SNAP | CRACK | BUZZ | RING | TICK | GLASS | WASH | BEND | SWELL | STUTTER | CRUSH | PHASE | GROWL | PERC | BELL | DARK | CHAOS |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Note | 36 | 37 | 38 | 39 | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 |

Formats de kit acceptés : le pack de la page (20 WAV, sans manifeste), un `.kit.zip`
avec un `kit.json` (schéma v1, cf. [la spec](docs/superpowers/specs/2026-10-22-kit-player-design.md))
qui peut renommer les pads et changer leurs notes, ou un dossier de WAV.

### Installation

- **Linux** : `make it drum.vst3` → `~/.vst3/`, `make it drum.clap` → `~/.clap/`.
- **Windows** (y compris Live sous Wine) : `make it drum.vst3` → `C:\Program Files\Common Files\VST3\`,
  `make it drum.clap` → `C:\Program Files\Common Files\CLAP\`.
- **macOS** : `make it drum.vst3` → `~/Library/Audio/Plug-Ins/VST3/`, `.clap` → `~/Library/Audio/Plug-Ins/CLAP/`.
  Le bundle n'est pas signé : clic droit → Ouvrir, ou `xattr -dr com.apple.quarantine`.

## Développement

```bash
cargo test --workspace                                   # 43 tests, dont un golden audio
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask bundle make-it-drum --release         # → target/bundled/
cargo xtask bundle make-it-drum --release --target x86_64-pc-windows-gnu   # cross via mingw
```

Structure :

- `crates/core` — cœur pur, sans hôte : `wav` (décodage), `kit` (pack web, `kit.json`, dossier, mapping note → pad), `voice` (one-shot interpolé), `engine` (20 voix, mixage). Tests dans `crates/core/tests/`, golden dans `tests/golden/`.
- `crates/plugin` — nih-plug : `slot` (partage GUI / fond / audio sans blocage), `loader` (état persistant, dossier des kits), `params`, `editor` (egui), `lib` (le `Plugin`).
- `xtask` — bundler nih-plug.

Contraintes : thread audio sans allocation (`assert_process_allocs` en debug) ni verrou bloquant ; TDD ; commits en français.

## Feuille de route

Voir la [proposition d'évolutions](https://github.com/yrbane/makeItDrum/blob/main/docs/proposals/2026-10-22-evolutions-vst-midi-ableton.md) du projet web : kits sauvegardés et `.kit.zip`, MIDI dans la page, export Drum Rack, « Envoyer dans Live ».

## Licence

MIT — © 2026 Masao « Saiya Geez ».

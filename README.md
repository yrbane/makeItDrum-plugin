# make it drum — kit player 🥁

> un projet de [Masao « Saiya Geez »](https://beacons.ai/saiyageez) · plugin par yrbane

Un instrument **VST3 / CLAP / Audio Unit** qui joue, depuis le MIDI, les kits de 20 percussions
fabriqués sur la page [make it drum](https://yrbane.github.io/makeItDrum/)
([dépôt](https://github.com/yrbane/makeItDrum)). Le son est strictement celui de la
page : ce sont les mêmes fichiers WAV. Le plugin ne génère rien, il joue.

- **Version** : 0.2.0 — voir [CHANGELOG.md](CHANGELOG.md)
- **Télécharger** : [releases](https://github.com/yrbane/makeItDrum-plugin/releases) — Linux (VST3, CLAP), Windows (VST3, CLAP), macOS universel (VST3, CLAP, AU)

## Utilisation

1. Sur la page make it drum, importez un son, puis **Exporter le pack** : vous obtenez
   `makeitdrum_pack.zip`.
2. Déposez ce fichier dans `~/Documents/makeitdrum/kits/` (ou dans le dossier donné
   par la variable `MAKEITDRUM_KITS`).
3. Dans votre DAW, ajoutez l'instrument **make it drum** : une instance neuve charge
   le premier kit du dossier. Changez de kit dans la selectbox (ou collez un chemin,
   ou déposez le `.zip` sur la fenêtre).
4. Jouez : pad 1 = note 36 (C1), pad 2 = 37, … pad 20 = 55. Un clic sur un pad le
   déclenche aussi. Le kit est embarqué dans le preset : le projet se rouvre sans le
   fichier.

| Recette | BOOM | KICK | THUD | SNAP | CRACK | BUZZ | RING | TICK | GLASS | WASH | BEND | SWELL | STUTTER | CRUSH | PHASE | GROWL | PERC | BELL | DARK | CHAOS |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Note | 36 | 37 | 38 | 39 | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 |

Les pads du pack de la page prennent les noms des recettes (BOOM, KICK, …).

Formats de kit acceptés : le pack de la page (20 WAV, sans manifeste), un `.kit.zip`
avec un `kit.json` (schéma v1, cf. [la spec](docs/superpowers/specs/2026-10-22-kit-player-design.md))
qui peut renommer les pads et changer leurs notes, ou un dossier de WAV.

### Installation

- **Linux** : `make it drum.vst3` → `~/.vst3/`, `make it drum.clap` → `~/.clap/`.
- **Windows** (y compris Live sous Wine) : `make it drum.vst3` → `C:\Program Files\Common Files\VST3\`,
  `make it drum.clap` → `C:\Program Files\Common Files\CLAP\`.
- **macOS** : `make it drum.vst3` → `~/Library/Audio/Plug-Ins/VST3/`, `.clap` → `~/Library/Audio/Plug-Ins/CLAP/`,
  `make it drum.component` → `~/Library/Audio/Plug-Ins/Components/` (Logic Pro, GarageBand, MainStage ;
  relancer la DAW, ou `killall -9 AudioComponentRegistrar` si l'instrument n'apparaît pas).
  Les bundles sont signés ad hoc, pas notarisés : après le téléchargement, `xattr -dr com.apple.quarantine`
  sur chacun, ou clic droit → Ouvrir.

### DAW couverts

| Format | Fichier | DAW |
|---|---|---|
| VST3 | `make it drum.vst3` | Ableton Live, Cubase / Nuendo, Studio One, FL Studio, Reaper, Bitwig, Ardour, Waveform, Renoise, Mixcraft… |
| CLAP | `make it drum.clap` | Bitwig, Reaper, FL Studio, Ardour, Waveform, Renoise, Mixcraft… |
| AU (macOS) | `make it drum.component` | Logic Pro, GarageBand, MainStage, plus toute DAW macOS qui charge des AU |

Pas d'AAX : Pro Tools demande le SDK Avid et une signature PACE, hors de portée d'un projet libre.
L'AU est produit par [clap-wrapper](https://github.com/free-audio/clap-wrapper) autour du CLAP et
validé par `auval` en CI ; il n'a pas encore été ouvert dans Logic par un humain, retours bienvenus.

## Développement

```bash
cargo test --workspace                                   # 43 tests, dont un golden audio
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask bundle make-it-drum --release         # → target/bundled/
cargo xtask bundle make-it-drum --release --target x86_64-pc-windows-gnu   # cross via mingw
cmake -S au -B au/build -DCMAKE_BUILD_TYPE=Release && cmake --build au/build   # AU (macOS, après bundle-universal)
cargo run --release --features standalone --bin make-it-drum -- --backend dummy   # fenêtre sans DAW
cargo run -p make-it-drum-core --example inspect -- ~/Documents/makeitdrum/kits/x.zip   # décrit un kit
```

Le binaire autonome (`--features standalone`) ouvre l'interface avec JACK, CPAL ou
sans audio (`--backend dummy`) : pratique pour tester un kit ou capturer l'écran.

Limite connue : avec clap-validator 0.4.1, les tests `state-reproducibility-*`
échouent sur ce plugin **comme sur l'exemple `gain` de nih-plug** (désaccord entre
nih-plug master et le validateur sur les paramètres) ; les autres tests passent et
les hôtes rechargent l'état correctement.

Structure :

- `crates/core` — cœur pur, sans hôte : `wav` (décodage), `kit` (pack web, `kit.json`, dossier, mapping note → pad), `voice` (one-shot interpolé), `engine` (20 voix, mixage). Tests dans `crates/core/tests/`, golden dans `tests/golden/`.
- `crates/plugin` — nih-plug : `slot` (partage GUI / fond / audio sans blocage), `loader` (état persistant, dossier des kits), `params`, `editor` (egui), `lib` (le `Plugin`).
- `xtask` — bundler nih-plug.
- `au` — gabarit CMake de l'Audio Unit (clap-wrapper, CLAP embarqué).

Contraintes : thread audio sans allocation (`assert_process_allocs` en debug) ni verrou bloquant ; TDD ; commits en français.

## Feuille de route

Voir la [proposition d'évolutions](https://github.com/yrbane/makeItDrum/blob/main/docs/proposals/2026-10-22-evolutions-vst-midi-ableton.md) du projet web : kits sauvegardés et `.kit.zip`, MIDI dans la page, export Drum Rack, « Envoyer dans Live ».

## Licence

MIT — © 2026 Masao « Saiya Geez ».

# make it drum — kit player 🥁

> un projet de [Masao « Saiya Geez »](https://beacons.ai/saiyageez) · plugin par yrbane

Un instrument **VST3 / CLAP / Audio Unit** qui fait ce que fait la page
[make it drum](https://yrbane.github.io/makeItDrum/) ([dépôt](https://github.com/yrbane/makeItDrum)),
mais dans la DAW : **n'importe quel son glissé dans le plugin devient un kit de 20 percussions**,
joué depuis le MIDI. Le moteur de la page (analyse spectrale, 20 recettes, graine par pad) est
porté en Rust ; le plugin lit aussi les kits exportés par la page.

- **Version** : 0.3.0 — voir [CHANGELOG.md](CHANGELOG.md)
- **Télécharger** : [releases](https://github.com/yrbane/makeItDrum-plugin/releases) — Linux (VST3, CLAP), Windows (VST3, CLAP), macOS universel (VST3, CLAP, AU)

## Utilisation

1. Dans votre DAW, ajoutez l'instrument **make it drum** sur une piste MIDI.
2. **Glissez un son dans la fenêtre** (zone du haut, ou n'importe où hors des pads) depuis le
   navigateur de la DAW (celui d'Ableton Live, par exemple) ou le Finder : WAV, AIFF, MP3, M4A/AAC,
   FLAC ou OGG. Un claquement de porte, une voix, une casserole… En une seconde environ,
   les 20 pads se remplissent : BOOM, KICK, THUD, … CHAOS, les 20 recettes de la page.
3. Jouez : pad 1 = note 36 (C1), pad 2 = 37, … pad 20 = 55. Un clic sur un pad le
   déclenche aussi, et le sélectionne : le panneau de droite règle **gain, hauteur,
   chute**, **sens de lecture**, **Cut itself** (une nouvelle frappe du pad coupe la
   précédente ; décoché, les frappes se superposent) et **Cut by all** (n'importe quel
   autre pad frappé coupe celui-ci). La ligne « Tous les pads » met ces deux options sur
   les 20 pads d'un coup. La vélocité MIDI est prise en compte. Le nom du pad se modifie
   dans le panneau.
   **Durée de la note** (coché par défaut) : le son s'arrête, en fondu court, quand la
   note MIDI est relâchée ; décoché, chaque frappe joue jusqu'au bout. Un clic sur un pad
   joue toujours jusqu'au bout.
4. **Clic droit sur un pad** : nouvelle variation de cette recette (nouvelle graine), comme
   le clic sur la page. Le bouton **Nouvelles variations** refait tout le kit. Chaque série
   de variations est mémorisée : **⏴** et **⏵** y reviennent (compteur « 3/7 »), et une
   nouvelle série après un retour en arrière remplace ce qui suivait.
5. **Glissez un son sur un pad** : ce pad seul prend ce son et son nom, et garde sa note.
   Les autres pads ne bougent pas.
6. **Fidélité**, **Durée** et **Départ** cadrent la génération pour reconnaître plus ou
   moins le son d'avant. À 0 %, c'est la page telle quelle. *Fidélité* ramène la hauteur
   vers celle de l'original, ouvre les filtres, atténue saturation, comb, reverb,
   bitcrush et compression, lit depuis le début du son et efface le renfort grave ;
   *Durée* étire palier et chute vers la durée du son (4 s au plus) ; *Départ* choisit où
   commence le son source, et **Au transitoire** le place juste avant l'attaque la plus
   forte. Relâcher un curseur rejoue les pads générés **avec leurs graines** : même idée
   de son, plus ou moins proche de l'original. Les réglages sont des paramètres du plugin
   (automatisables) et sont mémorisés dans le kit.
7. **Mélodique** : un seul pad joué sur tout le clavier, transposé (do médian = tel quel),
   pour une basse ou une ligne.
8. **Annuler** revient au kit précédent (8 niveaux). **Enregistrer** écrit le kit en
   `.kit.zip` dans le dossier des kits ; **Exporter** produit un dossier de WAV (à glisser
   d'un coup sur un Drum Rack de Live : les pads se remplissent à la suite) ou un
   **Drum Rack de Live** (`.adg` + WAV, notes reprises), dans
   `~/Documents/makeitdrum/exports/` (ou `MAKEITDRUM_EXPORTS`). L'export `.adg` est
   construit à partir d'un gabarit de Live 12 ; il n'a pas encore été ouvert dans Live
   par un humain.
9. **Capturer** : le plugin a une entrée audio « Capture » (sidechain). Dans Live, choisissez
   la source dans le panneau du plugin (« Audio From »), cliquez **● Capturer**, faites du
   bruit : le kit se fabrique à partir de l'enregistrement, sans fichier.
10. Le kit, ses graines, le son source et les réglages de pads sont embarqués dans le preset : le projet se
   rouvre sans les fichiers, et les variations restent possibles.
11. Les kits exportés par la page (`.kit.zip`, ou l'ancien `makeitdrum_pack.zip`) se chargent
   aussi : déposez-les dans `~/Documents/makeitdrum/kits/` (ou le dossier de la variable
   `MAKEITDRUM_KITS`) pour la selectbox, collez un chemin, ou glissez le `.zip` sur la fenêtre.
   Une instance neuve charge le premier kit du dossier, s'il y en a un.

| Recette | BOOM | KICK | THUD | SNAP | CRACK | BUZZ | RING | TICK | GLASS | WASH | BEND | SWELL | STUTTER | CRUSH | PHASE | GROWL | PERC | BELL | DARK | CHAOS |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Note | 36 | 37 | 38 | 39 | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 |

Les pads du pack de la page prennent les noms des recettes (BOOM, KICK, …).

Pour démarrer, `extras/rythmes/rythmes.py` écrit sept clips MIDI de 4 mesures sur ce
mapping (quatre au plancher, boom bap, breakbeat, half-time, polyrythme, chaos…) :
`python3 extras/rythmes/rythmes.py "<User Library de Live>/Clips/make it drum"`,
puis glissez un `.mid` depuis le navigateur de la DAW sur la piste du plugin.

### Le même moteur que la page, en Rust

Le son déposé est analysé (FFT, fondamentale, pic médium, centroïde, brillance), puis chaque
recette est rendue hors ligne : transitoire d'attaque, couches lues à offset et vitesse
variables, renfort de basses saturé, réseau d'allpass balayé par un LFO, comb en feedback,
saturation multi-étages et wavefolder, EQ de basses, reverb par convolution, compresseur,
bitcrush et normalisation de crête. L'aléatoire est le mulberry32 de la page, avec une graine
par pad : les tirages sont **identiques au bit près** à ceux de la page pour une graine donnée
(vérifié contre ses snapshots). Le rendu audio, lui, est une réécriture des nœuds Web Audio
(biquads selon la spécification, compresseur simplifié, sans suréchantillonnage des
waveshapers) : même recette, même caractère, pas la même forme d'onde que Chrome.

Formats de kit acceptés : le pack de la page (20 WAV, sans manifeste), un `.kit.zip`
avec un `kit.json` (schéma v1, cf. [la spec](docs/superpowers/specs/2026-10-22-kit-player-design.md))
qui peut renommer les pads, changer leurs notes et porter leurs graines, ou un dossier de WAV.
Un pad dont l'`index` saute des places laisse des pads vides entre deux. Un kit généré ou
modifié dans le plugin est resérialisé dans ce format (WAV flottant 32 bits, sans perte), avec
`source.wav` et `source.file` dans le manifeste pour pouvoir régénérer ; la page ignore ce
champ et lit le kit normalement.

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
cargo test --workspace                                   # 117 tests : golden audio, snapshots du moteur de la page
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask bundle make-it-drum --release         # → target/bundled/
cargo xtask bundle make-it-drum --release --target x86_64-pc-windows-gnu   # cross via mingw
cmake -S au -B au/build -DCMAKE_BUILD_TYPE=Release && cmake --build au/build   # AU (macOS, après bundle-universal)
cargo run --release --features standalone --bin make-it-drum -- --backend dummy   # fenêtre sans DAW
cargo run -p make-it-drum-core --example inspect -- ~/Documents/makeitdrum/kits/x.zip   # décrit un kit
cargo run --release -p make-it-drum-core --example render -- son.wav out/ 0.7 0.5 0      # les 20 pads en WAV, hors DAW
cargo run --release -p make-it-drum --example snapshot -- ui.png son.wav 2            # image de l'interface, sans hôte
```

Le binaire autonome (`--features standalone`) ouvre l'interface avec JACK, CPAL ou
sans audio (`--backend dummy`) : pratique pour tester un kit ou capturer l'écran.

Limite connue : avec clap-validator 0.4.1, les tests `state-reproducibility-*`
échouent sur ce plugin **comme sur l'exemple `gain` de nih-plug** (désaccord entre
nih-plug master et le validateur sur les paramètres) ; les autres tests passent et
les hôtes rechargent l'état correctement.

Structure :

- `crates/core` — cœur pur, sans hôte : `wav` (décodage WAV par hound, AIFF/MP3/FLAC/OGG/M4A par symphonia), `kit` (pack web, `kit.json`, dossier, mapping note → pad, génération, remplacement d'un pad, export `.kit.zip`), `voice` (one-shot interpolé), `engine` (20 voix, mixage). Le moteur porté de la page : `prng` (mulberry32), `analysis` (FFT, profil spectral), `curves` (tables de saturation, enveloppes), `flavors` (les 20 recettes), `dsp` (biquads Web Audio, retard, compresseur, convolution), `render` (`plan` tire les paramètres dans l'ordre de la page, `Shaping` les cadre après tirage sans changer l'ordre des tirages, `render` exécute le graphe). Tests dans `crates/core/tests/` : golden audio, valeurs de référence du JS (`pure.rs`), snapshots du renderer de la page pour la graine 42 (`render.rs`), fixture `tests/fixtures/source.wav` (celle de la page).
- `crates/plugin` — nih-plug : `slot` (partage GUI / fond / audio sans blocage), `loader` (état persistant, dossier des kits), `params`, `editor` (egui, dans l'esprit de la page : panneau beige, pads à écran LCD avec la forme d'onde), `history` (annuler), `variations` (séries de graines, ⏴ ⏵), `capture` (enregistrement depuis l'entrée auxiliaire, sans blocage du thread audio), `export` (`.kit.zip`, WAV, Drum Rack `.adg` depuis les gabarits de `templates/`), `lib` (le `Plugin`).
- `xtask` — bundler nih-plug.
- `au` — gabarit CMake de l'Audio Unit (clap-wrapper, CLAP embarqué).
- `vendor/baseview` — copie de la révision épinglée, avec les types de pasteboard que Live utilise pour ses fichiers (liste de chemins et `public.file-url`) et la réponse `Copy | Generic` au survol ; sans lui, Live n'autorise pas le dépôt.
- `extras/rythmes` — générateur de clips MIDI de démonstration.
- `vendor/egui-baseview` — copie de la révision épinglée par nih-plug, plus le relais des événements de glisser-déposer de baseview vers egui (`hovered_files`, `dropped_files`, `AcceptDrop`) ; sans lui, aucun dépôt n'atteint l'interface. Branché par `[patch]` dans `Cargo.toml`.

Contraintes : thread audio sans allocation (`assert_process_allocs` en debug) ni verrou bloquant ; TDD ; commits en français.

## Feuille de route

Voir la [proposition d'évolutions](https://github.com/yrbane/makeItDrum/blob/main/docs/proposals/2026-10-22-evolutions-vst-midi-ableton.md) du projet web : kits sauvegardés et `.kit.zip`, MIDI dans la page, export Drum Rack, « Envoyer dans Live ».

## Licence

MIT — © 2026 Masao « Saiya Geez ».

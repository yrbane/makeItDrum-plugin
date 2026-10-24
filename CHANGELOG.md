# Changelog

## 0.3.0 — 2026-10-23 · « Un son devient un kit, dans la DAW »

- **Le moteur de la page, porté en Rust** : glisser un son (WAV, AIFF, MP3, M4A/AAC, FLAC, OGG) dans la fenêtre fabrique les 20 percussions (BOOM … CHAOS) en une seconde environ, sur le thread de fond, les pads se remplissant au fur et à mesure. Analyse spectrale (FFT, fondamentale, pic médium, centroïde, brillance), les 20 recettes comme données, planification des tirages dans l'ordre exact de la page (mulberry32, graine par pad : **tirages identiques au bit près**, vérifiés contre les snapshots `renderer-BOOM/CHAOS/TICK` de la page), puis rendu : transitoire, couches, sub, phaser à LFO, comb (retard minimal d'un quantum comme Web Audio), saturation + wavefolder, EQ, reverb par convolution FFT, compresseur, bitcrush, normalisation.
- **Variations** : clic droit sur un pad = nouvelle graine pour cette recette ; bouton « Nouvelles variations des 20 pads ».
- **Glisser-déposer d'un son sur un pad** : remplace ce pad seul (nom du fichier, note conservée). Formats décodés par hound (WAV) et symphonia (le reste). Le pad visé s'entoure de vert pendant le survol.
- **Kit embarqué dans le preset** : kit, graines et son source (`source.wav` + `source.file` dans `kit.json` v1, ignoré par la page) resérialisés en `.kit.zip` dans l'état du plugin ; le projet se rouvre sans les fichiers et reste régénérable.
- **Pads vides** : un `kit.json` aux index non contigus place chaque pad à son index, les trous restent des pads vides (grisés, muets, qui ne répondent pas à leur note).
- **Correctif : le dépôt de fichiers ne fonctionnait pas.** egui-baseview, à la révision épinglée par nih-plug, ignore les événements de drag de baseview et refuse le dépôt côté macOS ; copie patchée dans `vendor/egui-baseview` (`[patch]`). Le dépôt d'un `.zip` annoncé en 0.1.0 marche donc à partir de cette version.
- **Fidélité et Durée** : deux paramètres du plugin (automatisables, 0 à 100 %) qui cadrent la génération pour reconnaître plus ou moins le son d'avant. Fidélité rapproche hauteur, filtres, saturation, comb, reverb, bitcrush, compression et offset de lecture du son d'origine et efface le renfort grave ; Durée étire palier et chute vers la durée du son (4 s au plus). Les tirages restent ceux de la page, dans le même ordre : relâcher un curseur rejoue les pads générés avec leurs graines (`Kit::reshape`), les pads déposés à la main ne bougent pas. Le cadrage est écrit dans `kit.json` (`shaping`, absent à zéro, ignoré par la page).
- **Correctif : la fenêtre était trop basse dans Live**, les deux dernières rangées de pads étaient coupées. Hauteur portée de 430 à 560 points ; la taille persistée change de clé (`editor-state-v2`) pour que les projets déjà sauvegardés reprennent la bonne hauteur.
- 81 tests, dont les valeurs de référence du JS de la page (PRNG, analyse, courbes, snapshots du renderer) et la fixture sonore de la page, décodée aussi en FLAC et en M4A (fixtures produites par afconvert).

## 0.2.0 — 2026-10-22 · « Audio Unit pour Logic et GarageBand »

- **Audio Unit v2** (`make it drum.component`) pour macOS, construit en CI avec [clap-wrapper](https://github.com/free-audio/clap-wrapper) autour du CLAP universel (arm64 + x86_64), CLAP embarqué dans le composant : Logic Pro, GarageBand et MainStage voient l'instrument (type `aumu`, fabricant `SaGz`, sous-type `MkDr`).
- **Validation `auval`** à chaque build macOS : la CI échoue si Apple refuse le composant.
- **Archive macOS** : `make-it-drum-macos-universal.zip` contient désormais VST3 + CLAP + AU ; gabarit CMake dans `au/`.
- README : tableau des DAW couverts par format (VST3, CLAP, AU) et installation du `.component`.
- `Cargo.lock` complété avec les dépendances du binaire autonome.

## 0.1.0 — 2026-10-22 · « Premier lecteur de kits »

- **Instrument VST3 et CLAP** (nih-plug, Rust) qui joue les kits de 20 percussions produits par [make it drum](https://github.com/yrbane/makeItDrum) depuis le MIDI : pad `i` → note General MIDI `36 + i` (BOOM = 36 … CHAOS = 55).
- **Sources de kits** : le `makeitdrum_pack.zip` exporté par la page (tel quel), un `.kit.zip` avec `kit.json` (schéma v1 : nom, recettes, notes par pad), ou un dossier de WAV.
- **20 voix one-shot**, polyphonie entre pads, redéclenchement dur, gain par vélocité, lecture au ratio d'échantillonnage par interpolation linéaire (WAV 16/24/32 bits ou flottant, mono ou stéréo).
- **Selectbox de kits** : les `.zip` de `~/Documents/makeitdrum/kits/` (ou `MAKEITDRUM_KITS`), chargement à la volée sur un thread de fond, champ chemin + « Charger », glisser-déposer d'un fichier.
- **État persistant** : chemin et octets du kit embarqués dans le preset, le projet se rouvre sans le fichier.
- **Interface egui** : crédit à Masao « Saiya Geez », version, gain, grille 5 × 4 de pads allumés pendant la lecture et cliquables.
- **Qualité** : cœur pur testé (43 tests, golden audio), thread audio sans allocation ni verrou bloquant, CI fmt + clippy + tests + bundles Linux, Windows et macOS, release automatique sur tag.

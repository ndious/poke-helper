# Pokédex interactif (Rust)

Base de données interactive des **1025 Pokémon** et du **catalogue complet des
cartes TCG** (20 635 cartes, physiques et digitales), 100 % offline une fois
les données générées.

Interface **web** navigable à la souris, plus une **TUI** plein écran
(`ratatui`) pour le terminal.

```console
$ cargo run --release          # interface web : http://127.0.0.1:8080
$ cargo run --release --bin pokedex   # TUI (terminal)
```

## L'interface web

`cargo run --release` démarre un serveur local (axum) ; ouvrez
http://127.0.0.1:8080. Tout se pilote à la souris :

- **Onglet Pokédex** — recherche instantanée (FR/EN, insensible aux accents),
  filtres type, **génération**, **intervalle de numéros** (min/max),
  légendaire/mythique, tri, pagination. **Sélection épinglée** : jusqu'à
  16 Pokémon marqués « + » restent affichés en chips sous les filtres,
  quelles que soient les recherches en cours (persistés dans le
  navigateur), avec un mode « sélection seule ». Le détail affiche la
  **génération**, les stats en barres et les faiblesses calculées
  (×4, ×2, ×½, ×¼, immunités).
- **Onglet Cartes TCG** — filtres **types** (cases à cocher, plusieurs
  types possibles : union), rareté, extension, **stade/sous-type** (tous
  ceux du catalogue : Base, Niveau 1/2, Gigamax, V, VSTAR, ex, GX…, avec
  leur effectif), **génération de l'espèce**
  (déduite du n° national), **intervalle de n° Pokédex**, **intervalle
  de PV** (toutes éditions et variantes ex/V/VMAX/GX confondues),
  **année de sortie** (min/max), tri (set, nom, n° Pokédex, PV, prix),
  tableau des cartes (nom, types, édition, n°, PV, rareté, année). Le
  détail montre les attaques, la légalité, la génération de l'espèce, le
  tableau de toutes les cartes de l'espèce, et un lien vers la fiche
  Pokédex.

Le port se change avec `PORT=3000 cargo run --release`.

API JSON (consommée par l'interface) :

| Route | Description |
|---|---|
| `GET /api/pokemons?q=&type=&gen=&min=&max=&legendary=&mythical=&sort=&page=` | Liste filtrée (min/max : intervalle de n°) |
| `GET /api/pokemons/{id}` | Fiche détaillée |
| `GET /api/cards?types=&rarity=&set=&gen=&min=&max=&hp_min=&hp_max=&year_min=&year_max=&stage=&sort=&page=` | Cartes filtrées (types : union de types d'énergie, séparés par virgules ; gen : espèce ; hp : PV toutes éditions ; year_min/year_max : années de sortie ; stage : n importe quel sous-type (Basic, Stage 1, VMAX…)) |
| `GET /api/cards/{id}` | Fiche d'une carte |
| `GET /api/meta` | Options des filtres (types, générations, raretés, extensions) |

## La TUI (terminal)

`cargo run --release --bin pokedex`. Deux onglets :

- **Pokédex** — les 1025 Pokémon : stats, faiblesses calculées depuis la table
  d'efficacité officielle, statuts légendaire/mythique.
- **Cartes TCG** — toutes les cartes du jeu de cartes (cartes physiques et
  versions digitales) : attaques, coûts, faiblesses, raretés, prix de marché,
  légalités, illustration. Les visuels des cartes ne sont pas chargés :
  les URLs restent disponibles dans les données (`images`).

### Touches

| Touche | Action |
|---|---|
| `Tab` / `←` `→` | Changer d'onglet |
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | Naviguer dans la liste |
| `/` | Recherche (nom FR/EN, insensible aux accents) |
| `t` | Filtrer par type (popup avec sa propre recherche) |
| `g` | Cycler les générations (Pokédex) |
| `l` / `m` | Légendaires / mythiques uniquement (Pokédex) |
| `r` | Filtrer par rareté (cartes) |
| `e` | Filtrer par extension (cartes) |
| `s` | Changer le tri (numéro, nom, PV, total, prix…) |
| `x` | Réinitialiser tous les filtres |
| `i` | Élargir le panneau de détail |
| `,` `.` | Faire défiler le détail |
| `q` / `Échap` | Quitter |

Sélectionner la valeur déjà active dans un popup la désactive (toggle).

## Données

```
data/raw/              CSV officiels PokéAPI (Pokémon : stats, types, noms FR…)
data/pokedex.json      1025 Pokémon compactés, embarqués dans le binaire
data/tcg_sets.json     176 extensions TCG (série, dates, légalités)
data/tcg_cards.json    Toutes les cartes TCG (19 Mo, chargées au démarrage)
```

### Régénérer les données

```console
$ cargo run --bin pokedex-importer            # CSV -> pokedex.json
$ cargo run --release --bin pokedex-tcg-fetch  # cartes TCG -> tcg_cards.json
$ cargo build                                   # re-embarque pokedex.json
```

Le fetcher des cartes télécharge les 176 fichiers d'extensions depuis le dépôt
GitHub `PokemonTCG/pokemon-tcg-data` (base de référence du projet pokemontcg.io,
bien plus stable que l'API REST, régulièrement en erreur 500). Les métadonnées
d'extensions et les prix de marché proviennent de l'API quand elle répond.
Le téléchargement est **résilient** : chaque extension validée est
sauvegardée dans `data/tcg_cards.partial2.json`, et un relancement reprend
là où il s'était arrêté.

Note : les prix de marché (TCGPlayer/Cardmarket) sont injectés par l'API en
temps réel ; si elle est indisponible, le catalogue reste complet mais la
ligne de prix est absente des fiches.

**Noms français des cartes** : le dépôt GitHub ne fournit que l'anglais. Le
fetcher génère donc `name_fr` en traduisant le nom d'espèce de chaque carte
Pokémon via le Pokédex (les suffixes ex, V, VMAX, GX… sont conservés) —
16 500+ cartes sont ainsi en français. Les cartes Dresseur/Énergie et les
variantes régionales restent en anglais.

## Déploiement sur fly.io

La configuration est prête (`fly.toml` + `Dockerfile` + `.dockerignore`).
L'image construit le serveur Rust en release et embarque les données TCG.

```console
$ fly auth login                     # flyctl : https://fly.io/docs/flyctl/
$ fly apps create ndious-pokedex     # nom unique, à reporter dans fly.toml
$ fly deploy
```

Détails :

- `fly.toml` référence `app = "ndious-pokedex"` — utilisez votre nom
  d'application et une région proche (`primary_region`, ici `cdg` = Paris).
- Le serveur lie `HOST=0.0.0.0` `PORT=8080` (requis par le proxy fly),
  configurés dans la section `[env]`.
- Machine minimale : `shared-cpu-1x`, 512 Mo (le catalogue fait ~19 Mo en
  mémoire vive au chargement).
- `auto_stop_machines` arrête la machine au repos et la réveille à la
  première requête (facturation à l'usage).
- Après un changement des données, régénérez `data/tcg_cards.json` puis
  relancez `fly deploy` : l'image embarque les données au build.

## Architecture

```
src/lib.rs             Modèle : Pokemon, Stats, TypeEntry, DexFile
src/db.rs              Index de recherche, filtres, efficacité des types
src/tcg.rs             Modèle des cartes TCG (champs API conservés via serde(flatten))
src/render.rs          Affichage texte (fiches, tableaux, comparaisons)
src/app.rs             État TUI : onglets, filtres, popups, navigation (testable)
src/ui.rs              Rendu ratatui de l'état
src/web.rs             API JSON (pokemons, cards, meta) consommée par l'interface web
src/bin/pokedex.rs     TUI (terminal)
src/bin/pokedex_web.rs Serveur web (axum) + interface web intégrée
web/index.html         Interface HTML/JS (embarquée dans le serveur)
src/bin/importer.rs    CSV PokéAPI -> pokedex.json
src/bin/tcg_fetch.rs   Fetcher résilient + noms FR des cartes
tests/commands.rs      Tests d'intégration Pokédex
tests/web_api.rs       Tests de l'API JSON du web
tests/tui.rs           Tests logique TUI + rendu (TestBackend)
tests/tcg_catalog.rs   Tests du catalogue TCG réel
```

## Tests

```console
$ cargo test
$ cargo clippy --all-targets --all-features --locked -- -D warnings
```

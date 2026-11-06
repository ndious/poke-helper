//! Modèle des cartes du TCG Pokémon (API pokemontcg.io).
//!
//! Le modèle capture les champs utiles à l'affichage et au filtrage ; tous
//! les autres champs de l'API sont conservés intacts dans `extra`
//! (`#[serde(flatten)]`) : aucune information n'est perdue.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::normalize;

/// Attaque d'une carte.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TcgAttack {
    pub name: String,
    #[serde(default)]
    pub cost: Vec<String>,
    #[serde(default, rename = "convertedEnergyCost")]
    pub converted_energy_cost: Option<u8>,
    #[serde(default)]
    pub damage: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Faiblesse ou résistance d'une carte.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TcgStatMod {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub value: String,
}

/// URLs des visuels (petite et haute résolution).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TcgImages {
    #[serde(default)]
    pub small: Option<String>,
    #[serde(default)]
    pub large: Option<String>,
}

/// Une carte TCG, physique ou digitale (les deux partagent la même fiche).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcgCard {
    pub id: String,
    pub name: String,
    /// Nom français (fichiers fr du dépôt GitHub), si disponible.
    #[serde(default, rename = "name_fr")]
    pub name_fr: Option<String>,
    #[serde(default)]
    pub supertype: String,
    #[serde(default)]
    pub subtypes: Vec<String>,
    /// PV en chaîne (« 140 »), absent pour les dresseurs/énergies.
    #[serde(default)]
    pub hp: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default, rename = "evolvesFrom")]
    pub evolves_from: Option<String>,
    #[serde(default)]
    pub attacks: Vec<TcgAttack>,
    #[serde(default)]
    pub weaknesses: Vec<TcgStatMod>,
    #[serde(default)]
    pub resistances: Vec<TcgStatMod>,
    #[serde(default, rename = "retreatCost")]
    pub retreat_cost: Vec<String>,
    #[serde(default, rename = "convertedRetreatCost")]
    pub converted_retreat_cost: Option<u8>,
    #[serde(rename = "set")]
    pub set_id: String,
    #[serde(default)]
    pub number: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default, rename = "flavorText")]
    pub flavor_text: Option<String>,
    #[serde(default, rename = "nationalPokedexNumbers")]
    pub national_pokedex_numbers: Vec<u32>,
    #[serde(default)]
    pub legalities: HashMap<String, String>,
    #[serde(default)]
    pub images: Option<TcgImages>,
    #[serde(default)]
    pub tcgplayer: Option<Value>,
    #[serde(default)]
    pub cardmarket: Option<Value>,
    /// Tous les autres champs de l'API, intacts.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl TcgCard {
    /// Nom affiché : français si connu, sinon anglais.
    pub fn display_name(&self) -> &str {
        self.name_fr.as_deref().unwrap_or(&self.name)
    }

    /// PV en nombre pour le tri.
    pub fn hp_value(&self) -> Option<u16> {
        self.hp.as_deref()?.parse().ok()
    }

    /// Prix de marché TCGPlayer (première variante trouvée).
    pub fn market_price(&self) -> Option<f64> {
        let prices = self.tcgplayer.as_ref()?.get("prices")?.as_object()?;
        for (_, variant) in prices {
            for key in ["market", "mid", "low"] {
                if let Some(v) = variant.get(key).and_then(Value::as_f64) {
                    if v > 0.0 {
                        return Some(v);
                    }
                }
            }
        }
        None
    }
}

/// Une extension (set) du TCG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcgSet {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub series: String,
    #[serde(default, rename = "printedTotal")]
    pub printed_total: u32,
    #[serde(default)]
    pub total: u32,
    #[serde(default, rename = "releaseDate")]
    pub release_date: String,
    #[serde(default)]
    pub images: Option<TcgImages>,
    #[serde(default, rename = "ptcgoCode")]
    pub ptcgo_code: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Critères de filtrage des cartes. Les intervalles sont bornes incluses.
#[derive(Debug, Clone, Default)]
pub struct CardFilter {
    /// Sous-chaîne sur le nom de la carte, de l'extension ou de la série.
    pub query: String,
    /// Types d'énergie : la carte doit en posséder au moins un.
    pub types: Vec<String>,
    pub rarity: Option<String>,
    pub set_id: Option<String>,
    pub supertype: Option<String>,
    /// Intervalle de numéro national de l'espèce (nationalPokedexNumbers).
    pub pokedex_min: Option<u32>,
    pub pokedex_max: Option<u32>,
    /// Génération de l'espèce (déduite du numéro national).
    pub generation: Option<u8>,
    /// Intervalle de PV, toutes éditions et variantes confondues.
    pub hp_min: Option<u16>,
    pub hp_max: Option<u16>,
    /// Intervalle d'année de sortie de l'extension, bornes incluses.
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// Stade d'évolution : « Basic », « Stage 1 » ou « Stage 2 ».
    pub stage: Option<String>,
}

/// Génération d'une espèce d'après son numéro national : les plages
/// sont contiguës (1-151 : gen 1, 152-251 : gen 2, etc.).
pub fn generation_of(national_id: u32) -> u8 {
    match national_id {
        1..=151 => 1,
        152..=251 => 2,
        252..=386 => 3,
        387..=493 => 4,
        494..=649 => 5,
        650..=721 => 6,
        722..=809 => 7,
        810..=905 => 8,
        _ => 9,
    }
}

/// Ordre de tri des listes de cartes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardSort {
    /// Ordre naturel de l'API (set, numéro).
    Api,
    Name,
    /// Numéro national de l'espèce.
    Number,
    /// PV décroissants.
    Hp,
    /// Prix de marché décroissant.
    Price,
}

impl CardSort {
    pub fn cycle(self) -> CardSort {
        match self {
            CardSort::Api => CardSort::Name,
            CardSort::Name => CardSort::Number,
            CardSort::Number => CardSort::Hp,
            CardSort::Hp => CardSort::Price,
            CardSort::Price => CardSort::Api,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CardSort::Api => "Set/numéro",
            CardSort::Name => "Nom",
            CardSort::Number => "N° Pokédex",
            CardSort::Hp => "PV décroissants",
            CardSort::Price => "Prix décroissant",
        }
    }
}

/// Catalogue des cartes et extensions.
pub struct TcgDatabase {
    cards: Vec<TcgCard>,
    sets: Vec<TcgSet>,
    sets_by_id: HashMap<String, usize>,
    /// Types d'énergie distincts triés.
    pub card_types: Vec<String>,
    /// Raretés distinctes triées.
    pub rarities: Vec<String>,
    /// Tous les sous-types distincts (stades et mécaniques : VMAX, VSTAR,
    /// GX…), avec le nombre de cartes, par effectif décroissant.
    pub stages: Vec<(String, usize)>,
    /// Supertypes distincts (Pokémon, Trainer, Energy).
    pub supertypes: Vec<String>,
}

#[derive(Deserialize)]
struct CardsFile {
    cards: Vec<TcgCard>,
}

#[derive(Deserialize)]
struct SetsFile {
    sets: Vec<TcgSet>,
}

impl TcgDatabase {
    /// Charge les fichiers générés par `pokedex-tcg-fetch`.
    /// Renvoie None si les fichiers sont absents ou illisibles.
    pub fn load_dir(dir: &std::path::Path) -> anyhow::Result<TcgDatabase> {
        let cards_raw = std::fs::read_to_string(dir.join("tcg_cards.json"))
            .map_err(anyhow::Error::from)
            .and_then(|s| Ok(serde_json::from_str::<CardsFile>(&s)?))?;
        let sets_raw = std::fs::read_to_string(dir.join("tcg_sets.json"))
            .map_err(anyhow::Error::from)
            .and_then(|s| Ok(serde_json::from_str::<SetsFile>(&s)?))?;
        Ok(TcgDatabase::new(cards_raw.cards, sets_raw.sets))
    }

    pub fn new(mut cards: Vec<TcgCard>, mut sets: Vec<TcgSet>) -> TcgDatabase {
        cards.sort_by_key(|c| (c.set_id.clone(), c.number.parse::<u32>().ok(), c.id.clone()));
        sets.sort_by(|a, b| a.release_date.cmp(&b.release_date));
        let sets_by_id = sets
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();

        // Valeurs brutes distinctes (servent de clés de filtrage) ;
        // la recherche dans les popups se fait, elle, en normalisé.
        let uniq = |getter: &dyn Fn(&TcgCard) -> Vec<String>| {
            let mut all: Vec<String> = cards.iter().flat_map(getter).collect();
            all.sort();
            all.dedup();
            all
        };

        let card_types = uniq(&|c| c.types.clone());
        let supertypes = uniq(&|c| vec![c.supertype.clone()]);
        let mut rarities: Vec<String> = cards.iter().filter_map(|c| c.rarity.clone()).collect();
        rarities.sort();
        rarities.dedup();

        // Tous les sous-types distincts avec leur effectif, par effectif
        // décroissant : alimente directement le filtre de l'interface.
        let mut subtype_counts: HashMap<String, usize> = HashMap::new();
        for card in &cards {
            for s in &card.subtypes {
                *subtype_counts.entry(s.clone()).or_default() += 1;
            }
        }
        let mut stages: Vec<(String, usize)> = subtype_counts.into_iter().collect();
        stages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        TcgDatabase {
            cards,
            sets,
            sets_by_id,
            card_types,
            rarities,
            stages,
            supertypes,
        }
    }

    pub fn cards(&self) -> &[TcgCard] {
        &self.cards
    }

    pub fn sets(&self) -> &[TcgSet] {
        &self.sets
    }

    pub fn set(&self, id: &str) -> Option<&TcgSet> {
        self.sets_by_id.get(id).map(|&i| &self.sets[i])
    }

    /// Indices des cartes correspondant aux critères, triées selon `sort`.
    /// Les indices sont stables (positions dans `cards`).
    pub fn filter_indices(&self, f: &CardFilter, sort: CardSort) -> Vec<usize> {
        let needle = normalize(f.query.trim());
        let mut rows: Vec<usize> = self
            .cards
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                (f.types.is_empty() || c.types.iter().any(|t| f.types.contains(t)))
                    && f.rarity
                        .as_ref()
                        .is_none_or(|r| c.rarity.as_deref() == Some(r.as_str()))
                    && f.set_id.as_ref().is_none_or(|s| c.set_id == *s)
                    && f.supertype.as_ref().is_none_or(|s| c.supertype == *s)
                    && f.pokedex_min
                        .is_none_or(|m| c.national_pokedex_numbers.iter().any(|&n| n >= m))
                    && f.pokedex_max
                        .is_none_or(|m| c.national_pokedex_numbers.iter().any(|&n| n <= m))
                    && f.generation.is_none_or(|g| {
                        c.national_pokedex_numbers
                            .iter()
                            .any(|&n| generation_of(n) == g)
                    })
                    && f.hp_min
                        .is_none_or(|m| c.hp_value().is_some_and(|hp| hp >= m))
                    && f.hp_max
                        .is_none_or(|m| c.hp_value().is_some_and(|hp| hp <= m))
                    && f.year_from
                        .is_none_or(|y| self.release_year(c).is_some_and(|yr| yr >= y))
                    && f.year_to
                        .is_none_or(|y| self.release_year(c).is_some_and(|yr| yr <= y))
                    && f.stage
                        .as_ref()
                        .is_none_or(|st| c.subtypes.iter().any(|s| s == st))
                    && (needle.is_empty()
                        || normalize(&c.name).contains(&needle)
                        || c.name_fr
                            .as_deref()
                            .is_some_and(|n| normalize(n).contains(&needle))
                        || self
                            .set(&c.set_id)
                            .is_some_and(|s| normalize(&s.name).contains(&needle)))
            })
            .map(|(i, _)| i)
            .collect();
        match sort {
            CardSort::Api => {}
            CardSort::Name => rows.sort_by(|&a, &b| {
                normalize(self.cards[a].display_name())
                    .cmp(&normalize(self.cards[b].display_name()))
            }),
            CardSort::Number => rows.sort_by_key(|&i| {
                std::cmp::Reverse(
                    self.cards[i]
                        .national_pokedex_numbers
                        .first()
                        .copied()
                        .unwrap_or(0),
                )
            }),
            CardSort::Hp => {
                rows.sort_by_key(|&i| std::cmp::Reverse(self.cards[i].hp_value().unwrap_or(0)))
            }
            CardSort::Price => rows.sort_by(|&a, &b| {
                self.cards[b]
                    .market_price()
                    .partial_cmp(&self.cards[a].market_price())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
        }
        rows
    }

    /// Année de sortie de l'extension d'une carte (premiers caractères de la
    /// date de sortie).
    fn release_year(&self, card: &TcgCard) -> Option<u16> {
        self.set(&card.set_id)
            .and_then(|s| s.release_date.get(0..4))
            .and_then(|y| y.parse().ok())
    }

    /// Cartes correspondant aux critères, triées selon `sort`.
    pub fn filter(&self, f: &CardFilter, sort: CardSort) -> Vec<&TcgCard> {
        self.filter_indices(f, sort)
            .into_iter()
            .map(|i| &self.cards[i])
            .collect()
    }

    /// Nombre de cartes d'une extension, pour l'affichage.
    pub fn set_card_count(&self, set_id: &str) -> usize {
        self.cards.iter().filter(|c| c.set_id == set_id).count()
    }
}

/// Normalisation dédiée à la recherche dans les popups.
pub fn match_label(needle: &str, label: &str) -> bool {
    normalize(needle.trim()).is_empty() || normalize(label).contains(&normalize(needle.trim()))
}

/// Libellé français d'un type d'énergie TCG.
pub fn energy_fr(en: &str) -> &str {
    match en {
        "Colorless" => "Incolore",
        "Darkness" => "Ténèbres",
        "Dragon" => "Dragon",
        "Fairy" => "Fée",
        "Fighting" => "Combat",
        "Fire" => "Feu",
        "Grass" => "Plante",
        "Lightning" => "Électrik",
        "Metal" => "Métal",
        "Psychic" => "Psy",
        "Water" => "Eau",
        other => other,
    }
}

/// Libellé français d'un sous-type de carte (stades et mécaniques).
/// Les termes sans équivalent établi restent tels quels.
pub fn subtype_fr(en: &str) -> &str {
    match en {
        "Basic" => "Base",
        "Stage 1" => "Niveau 1",
        "Stage 2" => "Niveau 2",
        "Baby" => "Bébé",
        "VMAX" => "Gigamax",
        "VSTAR" => "VSTAR",
        "MEGA" => "Méga",
        "Tera" => "Téra",
        "Ancient" => "Ancien",
        "Future" => "Futur",
        "Rapid Strike" => "Poing rapide",
        "Single Strike" => "Poing simple",
        "Fusion Strike" => "Poing fusion",
        "Ultra Beast" => "Ultra-Chimère",
        "Restored" => "Restauré",
        "Radiant" => "Radieux",
        "Item" => "Objet",
        "Pokémon Tool" => "Objet Pokémon",
        "Stadium" => "Stade",
        "Special" => "Spéciale",
        "Technical Machine" => "Machine technique",
        "Supporter" => "Supporter",
        "Prism Star" => "Prisme étoilé",
        "Star" => "Star",
        "Level-Up" => "Niveau supérieur",
        "LEVEL-UP" => "Niveau supérieur",
        other => other,
    }
}

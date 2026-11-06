//! Modèle de données du Pokédex embarqué.
//!
//! Le binaire `pokedex-importer` transforme les CSV de PokéAPI en un
//! `data/pokedex.json` compact, embarqué ici via `include_str!` : l'application
//! interactive fonctionne donc sans réseau ni dépendance externe.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub mod app;
pub mod db;
pub mod render;
pub mod tcg;
pub mod ui;
pub mod web;

pub use db::{normalize, Filter, Pokedex, SortKey, StatField};
pub use render::{card, compare, paginated, table, type_card};

/// Dimensions et seuil d'affichage des barres de stats.
pub const MAX_BASE_STAT: u16 = 255;

/// Statistiques de base d'un Pokémon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Stats {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub special_attack: u16,
    pub special_defense: u16,
    pub speed: u16,
}

impl Stats {
    /// Somme des six statistiques de base.
    pub fn total(&self) -> u32 {
        u32::from(self.hp)
            + u32::from(self.attack)
            + u32::from(self.defense)
            + u32::from(self.special_attack)
            + u32::from(self.special_defense)
            + u32::from(self.speed)
    }
}

/// Un Pokémon sous sa forme par défaut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pokemon {
    /// Identifiant national (1-1025).
    pub id: u32,
    pub name_fr: String,
    pub name_en: String,
    /// Catégorie, ex. « Pokémon Graine ».
    pub genus_fr: String,
    /// Génération d'apparition (1-9).
    pub generation: u8,
    /// Identifiants des types, ordre d'affichage (slot 1 puis slot 2).
    pub types: Vec<u32>,
    /// Taille en décimètres (10 dm = 1 m).
    pub height_dm: u32,
    /// Poids en hectogrammes (10 hg = 1 kg).
    pub weight_hg: u32,
    pub stats: Stats,
    /// Identifiant de la pré-évolution, si elle existe.
    #[serde(default)]
    pub evolves_from: Option<u32>,
    /// Identifiant de la lignée d'évolution (evolution_chain_id).
    #[serde(default)]
    pub evolution_chain: u32,
    pub is_legendary: bool,
    pub is_mythical: bool,
}

/// Un type offensif/défensif avec ses libellés français et anglais.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeEntry {
    pub id: u32,
    pub en: String,
    pub fr: String,
}

/// Format du fichier `data/pokedex.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexFile {
    pub types: Vec<TypeEntry>,
    /// Table d'efficacité : type attaquant -> type défenseur -> facteur
    /// (en % ; seules les entrées != 100 sont stockées).
    pub chart: HashMap<u32, HashMap<u32, u32>>,
    pub pokemons: Vec<Pokemon>,
}

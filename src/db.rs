//! Index de recherche et calculs d'efficacité des types.

use std::collections::HashMap;

use crate::{DexFile, Pokemon, Stats, TypeEntry, MAX_BASE_STAT};

/// Données du Pokédex chargées et indexées pour la recherche.
pub struct Pokedex {
    pokemons: Vec<Pokemon>,
    /// Types triés par identifiant.
    types: Vec<TypeEntry>,
    /// Identifiant de type -> libellés.
    types_by_id: HashMap<u32, TypeEntry>,
    /// Libellé normalisé (FR ou EN) -> identifiant de type.
    type_ids_by_label: HashMap<String, u32>,
    /// Table d'efficacité : attaquant -> défenseur -> facteur en %.
    chart: HashMap<u32, HashMap<u32, u32>>,
    /// Nom normalisé (FR ou EN) -> Pokémon.
    names_index: HashMap<String, u32>,
    by_id: HashMap<u32, usize>,
    /// Numéro national -> stade d'évolution (1 = base).
    stages: HashMap<u32, u8>,
    /// Espèces qui ont au moins une évolution.
    has_evolution: std::collections::HashSet<u32>,
    /// Lignée d'évolution -> membres, ordonnés par stade puis numéro.
    chains: HashMap<u32, Vec<u32>>,
}

/// Critères de filtrage pour la commande `liste`.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Identifiant du type requis (un seul type en argument).
    pub type_id: Option<u32>,
    pub generation: Option<u8>,
    pub legendary_only: bool,
    pub mythical_only: bool,
    /// Intervalle de numéros national, bornes incluses.
    pub id_min: Option<u32>,
    pub id_max: Option<u32>,
}

/// Tri disponible pour les listes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    /// Ordre du Pokédex national.
    Id,
    /// Nom français alphabétique.
    Name,
    /// Une statistique donnée, décroissante.
    Stat(StatField),
    /// Stade d'évolution croissant.
    Stage,
}

/// Les six statistiques, plus la somme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatField {
    Hp,
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
    Total,
}

impl StatField {
    /// Récupère la valeur correspondante.
    pub fn value(&self, s: &Stats) -> u32 {
        match self {
            StatField::Hp => u32::from(s.hp),
            StatField::Attack => u32::from(s.attack),
            StatField::Defense => u32::from(s.defense),
            StatField::SpecialAttack => u32::from(s.special_attack),
            StatField::SpecialDefense => u32::from(s.special_defense),
            StatField::Speed => u32::from(s.speed),
            StatField::Total => s.total(),
        }
    }

    /// Résout un alias saisi par l'utilisateur (accent et casse ignorés).
    pub fn parse(input: &str) -> Option<StatField> {
        match normalize(input).as_str() {
            "pv" | "hp" => Some(StatField::Hp),
            "attaque" | "atk" | "attack" => Some(StatField::Attack),
            "defense" | "def" | "defense-" => Some(StatField::Defense),
            "atq-spe" | "atqspec" | "attaque-speciale" | "spatk" | "spa" | "special-attack"
            | "atqspe" | "attaque-spe" => Some(StatField::SpecialAttack),
            "def-spe" | "defspe" | "defense-speciale" | "spdef" | "spd" | "special-defense"
            | "defense-spe" => Some(StatField::SpecialDefense),
            "vitesse" | "spe" | "speed" => Some(StatField::Speed),
            "total" | "base" => Some(StatField::Total),
            _ => None,
        }
    }

    /// Libellé court pour l'affichage.
    pub fn label(&self) -> &'static str {
        match self {
            StatField::Hp => "PV",
            StatField::Attack => "Attaque",
            StatField::Defense => "Défense",
            StatField::SpecialAttack => "Atq. spé",
            StatField::SpecialDefense => "Déf. spé",
            StatField::Speed => "Vitesse",
            StatField::Total => "Total",
        }
    }
}

/// Supprime les accents et met en minuscules, pour des recherches tolérantes
/// (é -> e, É -> e, œ -> oe, etc.).
pub fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => vec!['a'],
            'À' | 'Á' | 'Â' | 'Ä' | 'Ã' => vec!['a'],
            'è' | 'é' | 'ê' | 'ë' => vec!['e'],
            'È' | 'É' | 'Ê' | 'Ë' => vec!['e'],
            'ì' | 'í' | 'î' | 'ï' => vec!['i'],
            'Ì' | 'Í' | 'Î' | 'Ï' => vec!['i'],
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => vec!['o'],
            'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => vec!['o'],
            'ù' | 'ú' | 'û' | 'ü' => vec!['u'],
            'Ù' | 'Ú' | 'Û' | 'Ü' => vec!['u'],
            'ç' | 'Ç' => vec!['c'],
            'œ' | 'Œ' => vec!['o', 'e'],
            _ => vec![c.to_ascii_lowercase()],
        })
        .collect()
}

impl Pokedex {
    /// Charge le Pokédex embarqué dans le binaire.
    pub fn load_embedded() -> Result<Pokedex, serde_json::Error> {
        const RAW: &str = include_str!("../data/pokedex.json");
        Pokedex::from_json(RAW)
    }

    /// Charge depuis une chaîne JSON (utilisé aussi par les tests).
    pub fn from_json(raw: &str) -> Result<Pokedex, serde_json::Error> {
        let file: DexFile = serde_json::from_str(raw)?;
        Ok(Pokedex::from_file(file))
    }

    /// Construit les index à partir des données désérialisées.
    pub fn from_file(file: DexFile) -> Pokedex {
        let mut types = file.types;
        types.sort_by_key(|t| t.id);

        let mut type_ids_by_label = HashMap::new();
        let mut types_by_id = HashMap::new();
        for t in &types {
            type_ids_by_label.insert(normalize(&t.fr), t.id);
            type_ids_by_label.insert(normalize(&t.en), t.id);
            types_by_id.insert(t.id, t.clone());
        }

        let mut names_index = HashMap::new();
        let mut by_id = HashMap::new();
        for (i, p) in file.pokemons.iter().enumerate() {
            names_index.insert(normalize(&p.name_fr), p.id);
            names_index.insert(normalize(&p.name_en), p.id);
            by_id.insert(p.id, i);
        }

        // Évolution : stade (profondeur de la chaîne), parents, lignées.
        let mut stages = HashMap::new();
        let mut has_evolution = std::collections::HashSet::new();
        let mut chains: HashMap<u32, Vec<u32>> = HashMap::new();
        for p in &file.pokemons {
            if let Some(parent) = p.evolves_from {
                has_evolution.insert(parent);
            }
            let chain = chains.entry(p.evolution_chain).or_default();
            chain.push(p.id);
        }
        for members in chains.values_mut() {
            members.sort();
        }
        for p in &file.pokemons {
            let mut stage = 1;
            let mut current = p.id;
            // Les bébés ont des numéros supérieurs à leurs évolutions
            // (Pichu 172 > Pikachu 25) : le garde-fou suit les visites,
            // pas l'ordre des numéros.
            let mut visited = std::collections::HashSet::new();
            visited.insert(p.id);
            while let Some(Some(parent)) =
                file.pokemons.get(by_id[&current]).map(|x| x.evolves_from)
            {
                if !visited.insert(parent) {
                    break;
                }
                stage += 1;
                current = parent;
            }
            stages.insert(p.id, stage);
        }

        Pokedex {
            pokemons: file.pokemons,
            types,
            types_by_id,
            type_ids_by_label,
            chart: file.chart,
            names_index,
            by_id,
            stages,
            has_evolution,
            chains,
        }
    }

    /// Stade d'évolution : 1 = base, 2 = premier stade, etc. (profondeur de
    /// la chaîne de pré-évolutions, bébés inclus).
    pub fn stage(&self, id: u32) -> u8 {
        self.stages.get(&id).copied().unwrap_or(1)
    }

    /// Libellé du stade d'évolution (plafonné à « Niveau 2 » comme au TCG).
    pub fn stage_label(&self, id: u32) -> &'static str {
        match self.stage(id) {
            1 => "Base",
            2 => "Niveau 1",
            _ => "Niveau 2",
        }
    }

    /// Ce Pokémon a-t-il au moins une évolution ?
    pub fn has_evolution(&self, id: u32) -> bool {
        self.has_evolution.contains(&id)
    }

    /// Évolutions directes de ce Pokémon.
    pub fn evolutions(&self, id: u32) -> Vec<&Pokemon> {
        self.pokemons
            .iter()
            .filter(|p| p.evolves_from == Some(id))
            .collect()
    }

    /// Lignée complète (tous les membres de la chaîne d'évolution, soi
    /// inclus), ordonnés par stade puis par numéro.
    pub fn evolution_line(&self, id: u32) -> Vec<&Pokemon> {
        let chain = self
            .pokemons
            .get(self.by_id.get(&id).copied().unwrap_or(0))
            .map(|p| p.evolution_chain)
            .unwrap_or(0);
        self.chains
            .get(&chain)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&i| self.get(i))
                    .collect::<Vec<&Pokemon>>()
            })
            .unwrap_or_default()
    }

    pub fn pokemons(&self) -> &[Pokemon] {
        &self.pokemons
    }

    pub fn types(&self) -> &[TypeEntry] {
        &self.types
    }

    pub fn type_label(&self, id: u32) -> &str {
        self.types_by_id
            .get(&id)
            .map(|t| t.fr.as_str())
            .unwrap_or("?")
    }

    /// Résout un nom de type saisi (FR ou EN, accent-insensible).
    pub fn type_id(&self, label: &str) -> Option<u32> {
        self.type_ids_by_label.get(&normalize(label)).copied()
    }

    /// Facteur offensif (en %) d'un type attaquant sur un type défenseur.
    pub fn efficacy(&self, attacker: u32, defender: u32) -> u32 {
        self.chart
            .get(&attacker)
            .and_then(|m| m.get(&defender))
            .copied()
            .unwrap_or(100)
    }

    /// Résout un Pokémon par numéro national ou nom exact (FR/EN, accent-insensible).
    pub fn resolve(&self, query: &str) -> Option<&Pokemon> {
        let trimmed = query.trim();
        let id = if let Ok(num) = trimmed.parse::<u32>() {
            Some(num)
        } else if let Some(rest) = trimmed.strip_prefix('#') {
            rest.parse::<u32>().ok()
        } else {
            self.names_index.get(&normalize(trimmed)).copied()
        };
        id.and_then(|id| self.get(id))
    }

    /// Accès par identifiant national.
    pub fn get(&self, id: u32) -> Option<&Pokemon> {
        self.by_id.get(&id).map(|&i| &self.pokemons[i])
    }

    /// Recherche par sous-chaîne sur le nom FR ou EN (accent-insensible).
    /// Exclut le Pokémon exact déjà couvert par `resolve`.
    pub fn search(&self, query: &str) -> Vec<&Pokemon> {
        let needle = normalize(query.trim());
        if needle.is_empty() {
            return Vec::new();
        }
        self.pokemons
            .iter()
            .filter(|p| {
                normalize(&p.name_fr).contains(&needle) || normalize(&p.name_en).contains(&needle)
            })
            .collect()
    }

    /// Liste filtrée puis triée.
    pub fn list(&self, filter: &Filter, sort: SortKey) -> Vec<&Pokemon> {
        let mut result: Vec<&Pokemon> = self
            .pokemons
            .iter()
            .filter(|p| {
                filter.type_id.is_none_or(|t| p.types.contains(&t))
                    && filter.generation.is_none_or(|g| p.generation == g)
                    && filter.id_min.is_none_or(|m| p.id >= m)
                    && filter.id_max.is_none_or(|m| p.id <= m)
                    && (!filter.legendary_only || p.is_legendary)
                    && (!filter.mythical_only || p.is_mythical)
            })
            .collect();
        match sort {
            SortKey::Id => result.sort_by_key(|p| p.id),
            SortKey::Name => result.sort_by(|a, b| a.name_fr.cmp(&b.name_fr)),
            SortKey::Stat(f) => result.sort_by_key(|p| std::cmp::Reverse(f.value(&p.stats))),
            SortKey::Stage => result.sort_by_key(|p| (self.stage(p.id), p.id)),
        }
        result
    }

    /// Liste filtrée (critères + recherche texte FR/EN, accent-insensible),
    /// triée selon `sort`.
    pub fn query_list(&self, filter: &Filter, query: &str, sort: SortKey) -> Vec<&Pokemon> {
        let needle = normalize(query.trim());
        self.list(filter, sort)
            .into_iter()
            .filter(|p| {
                needle.is_empty()
                    || normalize(&p.name_fr).contains(&needle)
                    || normalize(&p.name_en).contains(&needle)
            })
            .collect()
    }

    /// Multiplicateur de dégâts du type attaquant sur ce Pokémon
    /// (produit des facteurs de ses types défensifs).
    pub fn defense_multiplier(&self, pokemon: &Pokemon, attacker: u32) -> f32 {
        pokemon
            .types
            .iter()
            .map(|&t| self.efficacy(attacker, t) as f32 / 100.0)
            .product()
    }

    /// Pour chaque type attaquant, le multiplicateur contre ce Pokémon.
    /// Trié par multiplicateur décroissant.
    pub fn multipliers(&self, pokemon: &Pokemon) -> Vec<(&TypeEntry, f32)> {
        let mut out: Vec<(&TypeEntry, f32)> = self
            .types
            .iter()
            .map(|t| (t, self.defense_multiplier(pokemon, t.id)))
            .collect();
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    /// Pokémon au hasard (générateur xorshift simple, sans dépendance).
    pub fn random(&self, seed: u64) -> Option<&Pokemon> {
        if self.pokemons.is_empty() {
            return None;
        }
        let mut x = seed | 1;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let idx = (x % self.pokemons.len() as u64) as usize;
        self.pokemons.get(idx)
    }

    /// Valeur max utile pour une statistique (pour les barres d'affichage).
    pub fn stat_max(field: StatField) -> u32 {
        if field == StatField::Total {
            720
        } else {
            u32::from(MAX_BASE_STAT)
        }
    }
}

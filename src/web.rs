//! API JSON consommée par l'interface web (`pokedex-web`).
//!
//! Les fonctions sont pures : elles prennent des paramètres désérialisés
//! et rendent du JSON, ce qui les rend testables sans serveur.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{Filter, Pokedex, SortKey, StatField};
use crate::tcg::{energy_fr, CardFilter, CardSort, TcgDatabase};
use crate::Pokemon;

/// Données partagées par tous les handlers.
pub struct WebData {
    pub dex: Pokedex,
    pub tcg: Option<TcgDatabase>,
}

const DEFAULT_PER_PAGE: usize = 50;
const MAX_PER_PAGE: usize = 500;

/// Paramètres de `/api/pokemons`. Tous les champs sont en `Option<String>` :
/// les paramètres vides ou invalides sont simplement ignorés.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PokemonParams {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default, rename = "type")]
    pub type_id: Option<String>,
    #[serde(default)]
    pub gen: Option<String>,
    #[serde(default)]
    pub legendary: Option<String>,
    #[serde(default)]
    pub mythical: Option<String>,
    /// Intervalle de numéro national : bornes incluses.
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
    /// id | name | total | hp | speed
    #[serde(default)]
    pub sort: Option<String>,
    /// Liste fixe de numéros nationaux, séparés par des virgules :
    /// ignore alors tous les autres filtres (sélection épinglée).
    #[serde(default)]
    pub ids: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
}

/// Paramètres de `/api/cards`, même logique de parsing tolérant.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CardParams {
    #[serde(default)]
    pub q: Option<String>,
    /// Types d'énergie, séparés par des virgules : la carte doit en
    /// posséder au moins un.
    #[serde(default, rename = "types")]
    pub types: Option<String>,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default, rename = "set")]
    pub set_id: Option<String>,
    /// Intervalle de numéro national de l'espèce.
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
    /// Génération de l'espèce.
    #[serde(default)]
    pub gen: Option<String>,
    /// Intervalle de PV.
    #[serde(default)]
    pub hp_min: Option<String>,
    #[serde(default)]
    pub hp_max: Option<String>,
    /// Intervalle d'année de sortie de l'extension.
    #[serde(default)]
    pub year_min: Option<String>,
    #[serde(default)]
    pub year_max: Option<String>,
    /// Stade d'évolution : basic | stage1 | stage2.
    #[serde(default)]
    pub stage: Option<String>,
    /// api | name | number | hp | price
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub per_page: Option<String>,
}

/// Option<String> -> Option<u32>, en ignorant les valeurs vides/invalides.
fn opt_num(s: &Option<String>) -> Option<u32> {
    s.as_deref().and_then(|v| v.trim().parse().ok())
}

fn opt_bool(s: &Option<String>) -> bool {
    matches!(s.as_deref(), Some("true") | Some("1"))
}

fn opt_page(s: &Option<String>) -> Option<usize> {
    opt_num(s).map(|v| v as usize)
}

fn parse_sort(sort: &Option<String>) -> SortKey {
    match sort.as_deref() {
        Some("name") => SortKey::Name,
        Some("total") => SortKey::Stat(StatField::Total),
        Some("hp") => SortKey::Stat(StatField::Hp),
        Some("speed") => SortKey::Stat(StatField::Speed),
        Some("stage") => SortKey::Stage,
        _ => SortKey::Id,
    }
}

fn paginate(total: usize, page: Option<usize>, per_page: Option<usize>) -> (usize, usize, usize) {
    let per_page = per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE);
    let pages = total.div_ceil(per_page).max(1);
    let page = page.unwrap_or(1).clamp(1, pages);
    (page, per_page, pages)
}

/// Vue résumée d'un Pokémon pour les listes : évolution plutôt que stats.
fn pokemon_summary(data: &WebData, p: &crate::Pokemon) -> Value {
    let evolutions: Vec<&str> = data
        .dex
        .evolutions(p.id)
        .iter()
        .map(|e| e.name_fr.as_str())
        .collect();
    let line: Vec<&str> = data
        .dex
        .evolution_line(p.id)
        .iter()
        .map(|x| x.name_fr.as_str())
        .collect();
    json!({
        "id": p.id,
        "name_fr": p.name_fr,
        "name_en": p.name_en,
        "types": p.types.iter().map(|&t| data.dex.type_label(t)).collect::<Vec<_>>(),
        "generation": p.generation,
        "legendary": p.is_legendary,
        "mythical": p.is_mythical,
        "stage": data.dex.stage_label(p.id),
        "stage_num": data.dex.stage(p.id),
        "evolves": data.dex.has_evolution(p.id),
        "evolutions": evolutions,
        "line": line.join(" → "),
    })
}

/// `GET /api/pokemons` : liste filtrée et paginée.
pub fn pokemons_json(data: &WebData, params: &PokemonParams) -> Value {
    // Mode sélection épinglée : renvoyer exactement les numéros demandés
    // (ceux qui existent), sans autre filtrage.
    let pinned: Option<Vec<u32>> = params.ids.as_deref().and_then(|ids| {
        let parsed: Vec<u32> = ids
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        (!parsed.is_empty()).then_some(parsed)
    });
    if let Some(ids) = pinned {
        let items: Vec<Value> = ids
            .iter()
            .filter_map(|&id| data.dex.get(id))
            .map(|p| pokemon_summary(data, p))
            .collect();
        let total = items.len();
        return json!({
            "total": total,
            "page": 1,
            "pages": 1,
            "per_page": total,
            "items": items,
        });
    }

    let filter = Filter {
        type_id: opt_num(&params.type_id),
        generation: opt_num(&params.gen).map(|g| g as u8),
        legendary_only: opt_bool(&params.legendary),
        mythical_only: opt_bool(&params.mythical),
        id_min: opt_num(&params.min),
        id_max: opt_num(&params.max),
    };
    let rows = data.dex.query_list(
        &filter,
        params.q.as_deref().unwrap_or(""),
        parse_sort(&params.sort),
    );
    let (page, per_page, pages) = paginate(
        rows.len(),
        opt_page(&params.page),
        opt_page(&params.per_page),
    );

    let items: Vec<Value> = rows
        .iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .map(|p| pokemon_summary(data, p))
        .collect();

    json!({
        "total": rows.len(),
        "page": page,
        "pages": pages,
        "per_page": per_page,
        "items": items,
    })
}

/// Efficacité des types regroupée par multiplicateur (×4, ×2, ×½, ×¼, ×0).
fn effectiveness_json(data: &WebData, p: &Pokemon) -> Value {
    let mut x4 = Vec::new();
    let mut x2 = Vec::new();
    let mut half = Vec::new();
    let mut quarter = Vec::new();
    let mut immune = Vec::new();
    for (t, m) in data.dex.multipliers(p) {
        match m {
            4.0 => x4.push(t.fr.clone()),
            2.0 => x2.push(t.fr.clone()),
            0.5 => half.push(t.fr.clone()),
            0.25 => quarter.push(t.fr.clone()),
            0.0 => immune.push(t.fr.clone()),
            _ => {}
        }
    }
    json!({
        "x4": x4, "x2": x2, "half": half, "quarter": quarter, "immune": immune,
    })
}

/// `GET /api/pokemons/{id}` : fiche détaillée, génération et évolution incluses.
pub fn pokemon_detail_json(data: &WebData, id: u32) -> Option<Value> {
    let p = data.dex.get(id)?;
    let evolves_from = p
        .evolves_from
        .and_then(|parent| data.dex.get(parent))
        .map(|parent| parent.name_fr.clone());
    let evolutions: Vec<String> = data
        .dex
        .evolutions(p.id)
        .iter()
        .map(|e| e.name_fr.clone())
        .collect();
    let line: Vec<String> = data
        .dex
        .evolution_line(p.id)
        .iter()
        .map(|x| x.name_fr.clone())
        .collect();
    Some(json!({
        "id": p.id,
        "name_fr": p.name_fr,
        "name_en": p.name_en,
        "genus_fr": p.genus_fr,
        "generation": p.generation,
        "types": p.types.iter().map(|&t| data.dex.type_label(t)).collect::<Vec<_>>(),
        "height_m": p.height_dm as f64 / 10.0,
        "weight_kg": p.weight_hg as f64 / 10.0,
        "stats": {
            "hp": p.stats.hp, "attack": p.stats.attack, "defense": p.stats.defense,
            "special_attack": p.stats.special_attack, "special_defense": p.stats.special_defense,
            "speed": p.stats.speed, "total": p.stats.total(),
        },
        "legendary": p.is_legendary,
        "mythical": p.is_mythical,
        "stage": data.dex.stage_label(p.id),
        "evolves": data.dex.has_evolution(p.id),
        "evolves_from": evolves_from,
        "evolutions": evolutions,
        "evolution_line": line,
        "effectiveness": effectiveness_json(data, p),
    }))
}

/// `GET /api/cards` : liste filtrée et paginée.
pub fn cards_json(data: &WebData, params: &CardParams) -> Value {
    let Some(tcg) = data.tcg.as_ref() else {
        return json!({ "error": "catalogue TCG indisponible", "total": 0, "page": 1, "pages": 1, "per_page": 0, "items": [] });
    };
    let filter = CardFilter {
        query: params.q.clone().unwrap_or_default(),
        types: params
            .types
            .as_deref()
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        rarity: params.rarity.clone().filter(|s| !s.is_empty()),
        set_id: params.set_id.clone().filter(|s| !s.is_empty()),
        pokedex_min: opt_num(&params.min),
        pokedex_max: opt_num(&params.max),
        generation: opt_num(&params.gen).map(|g| g as u8),
        hp_min: opt_num(&params.hp_min).map(|v| v as u16),
        hp_max: opt_num(&params.hp_max).map(|v| v as u16),
        year_from: opt_num(&params.year_min).map(|v| v as u16),
        year_to: opt_num(&params.year_max).map(|v| v as u16),
        // Alias historiques + n'importe quel sous-type du catalogue
        // (VMAX, VSTAR, GX, Basic…), passé tel quel.
        stage: match params.stage.as_deref() {
            Some("basic") => Some("Basic".into()),
            Some("stage1") => Some("Stage 1".into()),
            Some("stage2") => Some("Stage 2".into()),
            Some(other) => Some(other.to_owned()),
            None => None,
        },
        supertype: None,
    };
    let sort = match params.sort.as_deref() {
        Some("name") => CardSort::Name,
        Some("number") => CardSort::Number,
        Some("hp") => CardSort::Hp,
        Some("price") => CardSort::Price,
        _ => CardSort::Api,
    };
    let rows = tcg.filter_indices(&filter, sort);
    let (page, per_page, pages) = paginate(
        rows.len(),
        opt_page(&params.page),
        opt_page(&params.per_page),
    );

    let items: Vec<Value> = rows
        .iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .map(|&i| {
            let c = &tcg.cards()[i];
            let year = tcg
                .set(&c.set_id)
                .and_then(|s| s.release_date.get(0..4))
                .map(str::to_owned);
            json!({
                "id": c.id,
                "name": c.display_name(),
                "name_en": c.name,
                "subtypes": c.subtypes,
                "set": { "id": c.set_id, "name": tcg.set(&c.set_id).map(|s| s.name.clone()) },
                "number": c.number,
                "rarity": c.rarity,
                "hp": c.hp,
                "year": year,
                "types": c.types.iter().map(|t| energy_fr(t)).collect::<Vec<_>>(),
                "image": c.images.as_ref().and_then(|i| i.small.clone()),
            })
        })
        .collect();

    json!({
        "total": rows.len(),
        "page": page,
        "pages": pages,
        "per_page": per_page,
        "items": items,
    })
}

/// `GET /api/cards/{id}` : fiche détaillée d'une carte.
pub fn card_detail_json(data: &WebData, id: &str) -> Option<Value> {
    let tcg = data.tcg.as_ref()?;
    let c = tcg.cards().iter().find(|c| c.id == id)?;
    let set = tcg.set(&c.set_id);
    // Génération(s) de l'espèce, déduites des numéros nationaux.
    let mut generations: Vec<u8> = c
        .national_pokedex_numbers
        .iter()
        .map(|&n| crate::tcg::generation_of(n))
        .collect();
    generations.sort();
    generations.dedup();
    let attacks: Vec<Value> = c
        .attacks
        .iter()
        .map(|a| {
            json!({
                "name": a.name,
                "cost": a.cost.iter().map(|t| energy_fr(t)).collect::<Vec<_>>(),
                "damage": a.damage,
                "text": a.text,
            })
        })
        .collect();
    let weaknesses: Vec<Value> = c
        .weaknesses
        .iter()
        .map(|w| json!({ "type": energy_fr(&w.ty), "value": w.value }))
        .collect();
    let resistances: Vec<Value> = c
        .resistances
        .iter()
        .map(|r| json!({ "type": energy_fr(&r.ty), "value": r.value }))
        .collect();

    Some(json!({
        "id": c.id,
        "name": c.display_name(),
        "name_en": c.name,
        "supertype": c.supertype,
        "subtypes": c.subtypes,
        "hp": c.hp,
        "types": c.types.iter().map(|t| energy_fr(t)).collect::<Vec<_>>(),
        "evolves_from": c.evolves_from,
        "attacks": attacks,
        "weaknesses": weaknesses,
        "resistances": resistances,
        "retreat_cost": c.converted_retreat_cost,
        "number": c.number,
        "rarity": c.rarity,
        "generations": generations,
        "artist": c.artist,
        "flavor_text": c.flavor_text,
        "national_pokedex_numbers": c.national_pokedex_numbers,
        "legalities": c.legalities,
        "market_price": c.market_price(),
        "set": set.map(|s| json!({
            "id": s.id, "name": s.name, "series": s.series,
            "release_date": s.release_date, "total": s.total,
            "ptcgo_code": s.ptcgo_code,
        })),
        "images": {
            "small": c.images.as_ref().and_then(|i| i.small.clone()),
            "large": c.images.as_ref().and_then(|i| i.large.clone()),
        },
    }))
}

/// `GET /api/meta` : options des filtres déroulants.
pub fn meta_json(data: &WebData) -> Value {
    // (types de cartes, raretés, stades, extensions) si le catalogue est chargé.
    let (card_types, rarities, stages, sets): (Vec<Value>, Vec<String>, Vec<Value>, Vec<Value>) =
        match data.tcg.as_ref() {
            Some(tcg) => (
                tcg.card_types
                    .iter()
                    .map(|t| json!({ "key": t, "fr": energy_fr(t) }))
                    .collect(),
                tcg.rarities.clone(),
                tcg.stages
                    .iter()
                    .map(|(key, count)| {
                        json!({
                            "key": key,
                            "fr": crate::tcg::subtype_fr(key),
                            "count": count,
                        })
                    })
                    .collect(),
                tcg.sets()
                    .iter()
                    .map(|s| json!({ "id": s.id, "name": s.name, "series": s.series }))
                    .collect(),
            ),
            None => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
    json!({
        "types": data.dex.types().iter().map(|t| json!({
            "id": t.id, "fr": t.fr,
        })).collect::<Vec<_>>(),
        "generations": [1, 2, 3, 4, 5, 6, 7, 8, 9],
        "card_types": card_types,
        "rarities": rarities,
        "stages": stages,
        "sets": sets,
    })
}

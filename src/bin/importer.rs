//! Importeur : transforme les CSV de PokéAPI (data/raw/) en un
//! `data/pokedex.json` compact embarqué par le binaire interactif.
//!
//! Usage : `cargo run --bin pokedex-importer [chemin_csv] [sortie_json]`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use pokedex::{DexFile, Pokemon, Stats, TypeEntry};

const STANDARD_TYPES: u32 = 18;
const FRENCH_LANGUAGE_ID: u32 = 5;
const FORM_ID_THRESHOLD: u32 = 10_000;

fn load_csv(path: &Path) -> Result<Vec<csv::StringRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("lecture de {}", path.display()))?;
    reader
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("analyse de {}", path.display()))
}

/// Valeur optionnelle de colonne, vide si absente du CSV ou de la ligne.
fn field<'a>(row: &'a csv::StringRecord, index: usize, _name: &str) -> &'a str {
    row.get(index).unwrap_or_default()
}

fn required(row: &csv::StringRecord, index: usize, name: &str) -> Result<String> {
    let v = field(row, index, name).trim();
    if v.is_empty() {
        bail!("colonne « {name} » manquante dans {}", row.as_slice());
    }
    Ok(v.to_string())
}

fn parse_num(row: &csv::StringRecord, index: usize, name: &str) -> Result<u32> {
    required(row, index, name)?
        .parse::<u32>()
        .with_context(|| format!("valeur invalide dans la colonne « {name} » : {row:?}"))
}

/// Fait des données FR, avec repli sur l'anglais si absent.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn build_dex(raw: &Path) -> Result<DexFile> {
    // Types standards (1-18) avec libellés FR.
    let mut types = Vec::new();
    let mut fr_type_names: HashMap<u32, String> = HashMap::new();
    for row in load_csv(&raw.join("type_names.csv"))? {
        if parse_num(&row, 0, "type_id")? <= STANDARD_TYPES
            && parse_num(&row, 1, "local_language_id")? == FRENCH_LANGUAGE_ID
        {
            fr_type_names.insert(parse_num(&row, 0, "type_id")?, required(&row, 2, "name")?);
        }
    }
    for row in load_csv(&raw.join("types.csv"))? {
        let id = parse_num(&row, 0, "id")?;
        if id > STANDARD_TYPES {
            continue;
        }
        let en = required(&row, 1, "identifier")?;
        types.push(TypeEntry {
            id,
            fr: fr_type_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| capitalize(&en)),
            en,
        });
    }

    // Table d'efficacité, en ne gardant que les écarts à 100 %.
    let mut chart: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
    for row in load_csv(&raw.join("type_efficacy.csv"))? {
        let (atk, def, factor) = (
            parse_num(&row, 0, "damage_type_id")?,
            parse_num(&row, 1, "target_type_id")?,
            parse_num(&row, 2, "damage_factor")?,
        );
        if factor != 100 {
            chart.entry(atk).or_default().insert(def, factor);
        }
    }

    // Espèces : génération, statuts légendaire/mythique, évolution.
    struct SpeciesInfo {
        generation: u8,
        is_legendary: bool,
        is_mythical: bool,
        evolves_from: Option<u32>,
        evolution_chain: u32,
    }
    let mut species: HashMap<u32, SpeciesInfo> = HashMap::new();
    for row in load_csv(&raw.join("pokemon_species.csv"))? {
        species.insert(
            parse_num(&row, 0, "id")?,
            SpeciesInfo {
                generation: parse_num(&row, 2, "generation_id")? as u8,
                is_legendary: field(&row, 16, "is_legendary") == "1",
                is_mythical: field(&row, 17, "is_mythical") == "1",
                evolves_from: match field(&row, 3, "evolves_from_species_id") {
                    "" => None,
                    v => v.parse::<u32>().ok(),
                },
                evolution_chain: parse_num(&row, 4, "evolution_chain_id")?,
            },
        );
    }

    // Noms et catégories FR.
    let mut fr_names: HashMap<u32, (String, String)> = HashMap::new();
    for row in load_csv(&raw.join("pokemon_species_names.csv"))? {
        if parse_num(&row, 1, "local_language_id")? == FRENCH_LANGUAGE_ID {
            fr_names.insert(
                parse_num(&row, 0, "pokemon_species_id")?,
                (
                    required(&row, 2, "name")?,
                    field(&row, 3, "genus").trim().to_string(),
                ),
            );
        }
    }

    // Types par Pokémon.
    let mut pokemon_types: HashMap<u32, Vec<u32>> = HashMap::new();
    for row in load_csv(&raw.join("pokemon_types.csv"))? {
        let (pid, tid, slot) = (
            parse_num(&row, 0, "pokemon_id")?,
            parse_num(&row, 1, "type_id")?,
            parse_num(&row, 2, "slot")?,
        );
        let slots = pokemon_types.entry(pid).or_insert_with(|| vec![0, 0]);
        let idx = (slot.max(1) - 1) as usize;
        if idx < slots.len() {
            slots[idx] = tid;
        }
    }

    // Stats par Pokémon (stat_id 1=PV, 2=Atq, 3=Déf, 4=Atq spé, 5=Déf spé, 6=Vitesse).
    let mut pokemon_stats: HashMap<u32, Stats> = HashMap::new();
    for row in load_csv(&raw.join("pokemon_stats.csv"))? {
        let (pid, stat_id, value) = (
            parse_num(&row, 0, "pokemon_id")?,
            parse_num(&row, 1, "stat_id")?,
            parse_num(&row, 2, "base_stat")? as u16,
        );
        let stats = pokemon_stats.entry(pid).or_default();
        match stat_id {
            1 => stats.hp = value,
            2 => stats.attack = value,
            3 => stats.defense = value,
            4 => stats.special_attack = value,
            5 => stats.special_defense = value,
            6 => stats.speed = value,
            _ => {}
        }
    }

    // Formes par défaut uniquement, index national 1-1025.
    let mut pokemons = Vec::new();
    for row in load_csv(&raw.join("pokemon.csv"))? {
        let id = parse_num(&row, 0, "id")?;
        if id >= FORM_ID_THRESHOLD || field(&row, 7, "is_default") != "1" {
            continue;
        }
        let species_id = parse_num(&row, 2, "species_id")?;
        let info = match species.get(&species_id) {
            Some(i) => i,
            None => {
                eprintln!("avertissement : espèce {species_id} inconnue, Pokémon {id} ignoré");
                continue;
            }
        };
        let name_en = capitalize(&required(&row, 1, "identifier")?);
        let (name_fr, genus_fr) = fr_names
            .get(&species_id)
            .cloned()
            .unwrap_or_else(|| (capitalize(&name_en), String::new()));
        let stats = pokemon_stats.get(&id).copied().unwrap_or_default();
        if stats == Stats::default() {
            eprintln!("avertissement : stats manquantes pour le Pokémon {id}");
        }
        let types = pokemon_types
            .get(&id)
            .map(|slots| slots.iter().copied().filter(|&t| t != 0).collect())
            .unwrap_or_default();
        pokemons.push(Pokemon {
            id,
            name_fr,
            name_en,
            genus_fr,
            generation: info.generation,
            types,
            height_dm: parse_num(&row, 3, "height")?,
            weight_hg: parse_num(&row, 4, "weight")?,
            stats,
            evolves_from: info.evolves_from,
            evolution_chain: info.evolution_chain,
            is_legendary: info.is_legendary,
            is_mythical: info.is_mythical,
        });
    }
    pokemons.sort_by_key(|p| p.id);

    Ok(DexFile {
        types,
        chart,
        pokemons,
    })
}

fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let raw_dir = args
        .first()
        .map_or_else(|| PathBuf::from("data/raw"), |p| p.clone());
    let out_path = args
        .get(1)
        .map_or_else(|| PathBuf::from("data/pokedex.json"), |p| p.clone());

    let dex = build_dex(&raw_dir)?;
    let json = serde_json::to_string(&dex)?;
    std::fs::write(&out_path, json)
        .with_context(|| format!("écriture de {}", out_path.display()))?;
    println!(
        "{} Pokémon, {} types → {} ({} octets)",
        dex.pokemons.len(),
        dex.types.len(),
        out_path.display(),
        std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0)
    );
    Ok(())
}

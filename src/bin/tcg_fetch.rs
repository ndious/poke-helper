//! Téléchargement complet du catalogue de cartes TCG Pokémon.
//!
//! Source principale : le dépôt GitHub `PokemonTCG/pokemon-tcg-data`, base de
//! données de référence du projet pokemontcg.io (toutes les cartes imprimées,
//! avec leurs visuels et légalités — donc aussi leur version digitale
//! TCG Live). Les extensions (sets) et les prix de marché proviennent de
//! l'API pokemontcg.io, en tolérant son indisponibilité.
//!
//! Résilient : chaque extension validée est sauvegardée dans
//! `data/tcg_cards.partial.json` ; un relancement reprend là où il
//! s'était arrêté.
//!
//! Usage : `cargo run --release --bin pokedex-tcg-fetch`
//! Sortie : data/tcg_sets.json + data/tcg_cards.json

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

const GITHUB_RAW: &str = "https://raw.githubusercontent.com/PokemonTCG/pokemon-tcg-data/master";
const API: &str = "https://api.pokemontcg.io/v2";
const MAX_ATTEMPTS: u32 = 6;
const DELAY: Duration = Duration::from_millis(150);

/// GET JSON avec réessai à backoff exponentiel sur les erreurs passagères.
/// `retry_5xx` désactive la persévérance quand une erreur 5xx doit être
/// immédiatement remontée (changement de source, par exemple).
fn get_json(url: &str, retry_5xx: bool) -> Result<Value> {
    let mut delay = Duration::from_secs(1);
    for attempt in 1..=MAX_ATTEMPTS {
        match ureq::get(url).call() {
            Ok(resp) => {
                return resp
                    .into_json::<Value>()
                    .with_context(|| format!("réponse non-JSON de {url}"));
            }
            Err(ureq::Error::Status(code, _)) => {
                if retry_5xx && (code == 429 || code >= 500) {
                    eprintln!(
                        "  HTTP {code}, nouvel essai {attempt}/{MAX_ATTEMPTS} dans {delay:?}…"
                    );
                    sleep(delay);
                    delay = (delay * 2).min(Duration::from_secs(30));
                } else {
                    return Err(anyhow::anyhow!("HTTP {code} sur {url}"));
                }
            }
            Err(e) => return Err(anyhow::anyhow!("requête vers {url} : {e}")),
        }
    }
    Err(anyhow::anyhow!(
        "abandon après {MAX_ATTEMPTS} essais : {url}"
    ))
}

/// Écriture atomique : d'abord un fichier temporaire, puis renommage.
fn write_json(path: &Path, value: &Value) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(value)?)
        .with_context(|| format!("écriture de {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renommage vers {}", path.display()))?;
    Ok(())
}

fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Clé de recherche d'un mot d'un nom de carte : minuscules ASCII,
/// sans ponctuation.
fn name_key(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect()
}

/// Génère `name_fr` pour les cartes Pokémon : le dépôt GitHub ne fournit
/// que l'anglais, mais la quasi-totalité des cartes portent le nom d'une
/// espèce (PokéAPI, lui, a les noms français) suivi d'un suffixe conservé
/// (ex, V, VMAX, GX…). Les cartes Dresseur/Énergie restent en anglais.
fn frenchify(cards: &mut [Value]) {
    let dex = match pokedex::db::Pokedex::load_embedded() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Pokédex embarqué indisponible ({e}) : pas de noms FR");
            return;
        }
    };
    let species: HashMap<String, String> = dex
        .pokemons()
        .iter()
        .map(|p| (name_key(&p.name_en), p.name_fr.clone()))
        .collect();

    let mut translated = 0usize;
    for card in cards.iter_mut() {
        if card["supertype"].as_str() != Some("Pokémon") {
            continue;
        }
        let Some(name) = card["name"].as_str().map(str::to_owned) else {
            continue;
        };
        let mut out: Vec<String> = Vec::new();
        let mut changed = false;
        for token in name.split(' ') {
            // « Charizard-EX » -> base « Charizard » + « -EX » conservé.
            let (base, rest) = match token.find(['-', '.']) {
                Some(i) => (&token[..i], &token[i..]),
                None => (token, ""),
            };
            if let Some(fr) = species.get(&name_key(base)) {
                out.push(format!("{fr}{rest}"));
                changed = true;
            } else if let Some(fr) = species.get(&name_key(token)) {
                out.push(fr.clone());
                changed = true;
            } else {
                out.push(token.to_owned());
            }
        }
        if changed {
            card["name_fr"] = Value::String(out.join(" "));
            translated += 1;
        }
    }
    eprintln!("Noms français générés pour {translated} cartes");
}

/// Métadonnées des extensions, via l'API (tolérante à l'indisponibilité) :
/// en cas d'échec, le fichier existant est conservé tel quel.
fn fetch_sets(out_dir: &Path) {
    eprintln!("Téléchargement des extensions…");
    match get_json(&format!("{API}/sets?pageSize=250"), true) {
        Ok(body) => {
            let sets = json!({ "sets": body["data"].clone(), "fetched_at": epoch_seconds() });
            let count = sets["sets"].as_array().map(Vec::len).unwrap_or(0);
            if count > 0 && write_json(&out_dir.join("tcg_sets.json"), &sets).is_ok() {
                eprintln!("  {count} extensions");
            }
        }
        Err(e) => eprintln!("  échec ({e}) : le fichier existant est conservé"),
    }
}

/// Identifiants des extensions, soit via l'API, soit en listant le dépôt
/// GitHub (cards/en/*.json).
fn set_ids() -> Result<Vec<String>> {
    if let Ok(body) = get_json(&format!("{API}/sets?pageSize=250"), false) {
        let ids: Vec<String> = body["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s["id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if !ids.is_empty() {
            return Ok(ids);
        }
    }
    eprintln!("API indisponible : liste des extensions via GitHub…");
    let body = get_json(
        "https://api.github.com/repos/PokemonTCG/pokemon-tcg-data/contents/cards/en",
        true,
    )?;
    let ids: Vec<String> = body
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|f| f["name"].as_str())
                .filter(|n| n.ends_with(".json"))
                .map(|n| n.trim_end_matches(".json").to_owned())
                .collect()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return Err(anyhow::anyhow!("aucune extension trouvée"));
    }
    Ok(ids)
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Partial {
    cards: Vec<Value>,
    /// Identifiants des extensions déjà traitées.
    done: Vec<String>,
}

fn main() -> Result<()> {
    let out_dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("data"), PathBuf::from);
    // Point de contrôle par extension (les anciens formats par page de
    // l'API pokemontcg.io ne sont pas réutilisables : on repart de zéro).
    let partial_path = out_dir.join("tcg_cards.partial2.json");
    let _ = std::fs::remove_file(out_dir.join("tcg_cards.partial.json"));

    fetch_sets(&out_dir);
    let ids = set_ids()?;
    eprintln!("{} extensions à télécharger", ids.len());

    let mut state = match std::fs::read_to_string(&partial_path) {
        Ok(raw) => serde_json::from_str::<Partial>(&raw)
            .with_context(|| format!("reprise de {}", partial_path.display()))?,
        Err(_) => Partial {
            cards: Vec::new(),
            done: Vec::new(),
        },
    };
    let done: std::collections::HashSet<String> = state.done.iter().cloned().collect();
    eprintln!("Reprise : {} cartes déjà enregistrées", state.cards.len());

    for (i, set_id) in ids.iter().enumerate() {
        if done.contains(set_id) {
            continue;
        }
        let url = format!("{GITHUB_RAW}/cards/en/{set_id}.json");
        let cards = match get_json(&url, true) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(e) => {
                eprintln!("  échec {set_id} ({e}) : interrompu, relancer pour reprendre");
                // Point de contrôle conservé : les sets déjà faits ne
                // seront pas retéléchargés.
                return Err(e);
            }
        };
        let before = state.cards.len();
        for mut card in cards {
            card["set"] = Value::String(set_id.clone());
            state.cards.push(card);
        }
        state.done.push(set_id.clone());
        eprintln!(
            "  [{}/{}] {} : {} cartes (total : {})",
            i + 1,
            ids.len(),
            set_id,
            state.cards.len() - before,
            state.cards.len()
        );
        write_json(&partial_path, &serde_json::to_value(&state)?)?;
        sleep(DELAY);
    }

    frenchify(&mut state.cards);
    let cards_file = json!({ "cards": state.cards, "fetched_at": epoch_seconds() });
    let cards_path = out_dir.join("tcg_cards.json");
    write_json(&cards_path, &cards_file)?;
    let size = std::fs::metadata(&cards_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "{} cartes ({:.1} Mo), {} extensions → {}",
        state.cards.len(),
        size as f64 / 1_000_000.0,
        state.done.len(),
        cards_path.display()
    );
    std::fs::remove_file(&partial_path).ok();
    Ok(())
}

//! Tests d'intégration sur le dataset réel embarqué.

use pokedex::db::{Filter, Pokedex, SortKey, StatField};

fn dex() -> Pokedex {
    Pokedex::load_embedded().expect("le dataset embarqué doit être valide")
}

#[test]
fn dataset_should_contain_1025_pokemons_and_18_types() {
    let dex = dex();
    assert_eq!(dex.pokemons().len(), 1025);
    assert_eq!(dex.types().len(), 18);
    assert_eq!(dex.pokemons().first().expect("non vide").id, 1);
    assert_eq!(dex.pokemons().last().expect("non vide").id, 1025);
}

#[test]
fn resolve_should_accept_id_hash_and_names_with_accents() {
    let dex = dex();
    let pikachu = dex.resolve("pikachu").expect("nom EN");
    assert_eq!(pikachu.id, 25);
    assert!(dex.resolve("Pikachu").is_some());
    assert!(dex.resolve("25").is_some());
    assert!(dex.resolve("#25").is_some());
    let dracaufeu = dex.resolve("Dracaufeu").expect("nom FR");
    assert_eq!(dracaufeu.id, 6);
    // Insensible à la casse et aux accents.
    assert!(dex.resolve("dracaufeu").is_some());
    assert!(dex.resolve("DRACAUFEU").is_some());
}

#[test]
fn search_should_match_substring_in_both_languages() {
    let dex = dex();
    let rows = dex.search("pika");
    assert!(rows.iter().any(|p| p.id == 25));
    // Élektek est le nom FR d'Electabuzz : la recherche doit traverser
    // les accents et la casse.
    let rows = dex.search("élekt");
    assert!(rows.iter().any(|p| p.name_en == "Electabuzz"));
}

#[test]
fn filter_should_combine_type_and_generation() {
    let dex = dex();
    let filter = Filter {
        type_id: dex.type_id("plante"),
        generation: Some(1),
        ..Filter::default()
    };
    let rows = dex.list(&filter, SortKey::Id);
    assert_eq!(rows.len(), 14);
    assert!(rows.iter().all(|p| p.generation == 1));
}

#[test]
fn legendary_and_mythical_flags_should_be_accurate() {
    let dex = dex();
    let bulbi = dex.resolve("bulbasaur").expect("bulbizarre");
    assert!(!bulbi.is_legendary && !bulbi.is_mythical);
    let mew = dex.resolve("mew").expect("mew");
    assert!(mew.is_mythical && !mew.is_legendary);
    let mewtwo = dex.resolve("mewtwo").expect("mewtwo");
    assert!(mewtwo.is_legendary);
}

#[test]
fn efficacy_should_follow_official_type_chart() {
    let dex = dex();
    let fire = dex.type_id("feu").expect("type feu");
    let grass = dex.type_id("plante").expect("type plante");
    let water = dex.type_id("eau").expect("type eau");
    let ground = dex.type_id("sol").expect("type sol");
    assert_eq!(dex.efficacy(fire, grass), 200);
    assert_eq!(dex.efficacy(fire, water), 50);
    assert_eq!(dex.efficacy(fire, fire), 50);
    assert_eq!(dex.efficacy(ground, dex.type_id("vol").expect("vol")), 0);
}

#[test]
fn charizard_should_be_4x_weak_to_rock_and_immune_to_ground() {
    let dex = dex();
    let dracaufeu = dex.resolve("dracaufeu").expect("dracaufeu");
    let rock = dex.type_id("roche").expect("roche");
    let ground = dex.type_id("sol").expect("sol");
    assert_eq!(dex.defense_multiplier(dracaufeu, rock), 4.0);
    assert_eq!(dex.defense_multiplier(dracaufeu, ground), 0.0);
}

#[test]
fn multipliers_should_be_sorted_by_descending_factor() {
    let dex = dex();
    let dracaufeu = dex.resolve("dracaufeu").expect("dracaufeu");
    let factors: Vec<f32> = dex
        .multipliers(dracaufeu)
        .into_iter()
        .map(|(_, m)| m)
        .collect();
    assert!(
        factors.windows(2).all(|w| w[0] >= w[1]),
        "les facteurs doivent être décroissants"
    );
    assert_eq!(factors[0], 4.0);
}

#[test]
fn top_total_should_rank_arceus_first() {
    let dex = dex();
    let rows = dex.list(&Filter::default(), SortKey::Stat(StatField::Total));
    assert_eq!(rows[0].name_en, "Arceus");
    assert_eq!(rows[0].stats.total(), 720);
}

#[test]
fn stat_field_should_parse_french_and_english_aliases() {
    assert_eq!(StatField::parse("vitesse"), Some(StatField::Speed));
    assert_eq!(StatField::parse("PV"), Some(StatField::Hp));
    assert_eq!(StatField::parse("atq-spe"), Some(StatField::SpecialAttack));
    assert_eq!(StatField::parse("nimporte"), None);
}

#[test]
fn normalize_should_strip_accents_and_case() {
    assert_eq!(pokedex::normalize("Électrik"), "electrik");
    assert_eq!(pokedex::normalize("Ténèbres"), "tenebres");
    assert_eq!(pokedex::normalize("Fée"), "fee");
}

#[test]
fn render_card_and_compare_should_mention_names_and_weaknesses() {
    let dex = dex();
    let dracaufeu = dex.resolve("dracaufeu").expect("dracaufeu");
    let card = pokedex::render::card(&dex, dracaufeu);
    assert!(card.contains("Dracaufeu"));
    assert!(card.contains("Feu / Vol"));
    assert!(card.contains("Roche"));
    let compare =
        pokedex::render::compare(&dex, dracaufeu, dex.resolve("tortank").expect("tortank"));
    assert!(compare.contains("Dracaufeu"));
    assert!(compare.contains("Tortank"));
}

//! Tests d'intégration sur le catalogue TCG réel (pokemontcg.io).
//! Ignorés silencieusement si le catalogue n'a pas encore été téléchargé.

use pokedex::tcg::{CardFilter, CardSort, TcgDatabase};

fn tcg() -> Option<TcgDatabase> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    match TcgDatabase::load_dir(&dir) {
        Ok(db) => Some(db),
        Err(e) => {
            eprintln!("catalogue TCG absent ({e}) : tests ignorés");
            None
        }
    }
}

#[test]
fn catalog_should_be_complete_and_consistent() {
    let Some(db) = tcg() else { return };
    assert!(db.cards().len() > 20_000, "catalogue complet attendu");
    assert_eq!(db.sets().len(), 176);
    // Chaque carte référence un set connu.
    let orphan = db
        .cards()
        .iter()
        .find(|c| db.set(&c.set_id).is_none())
        .map(|c| c.id.clone());
    assert_eq!(orphan, None, "carte sans extension connue");
}

#[test]
fn charizard_base_set_should_have_expected_fields() {
    let Some(db) = tcg() else { return };
    let zard = db
        .cards()
        .iter()
        .find(|c| c.id == "base1-4")
        .expect("Dracaufeu de la série de base (base1-4)");
    assert_eq!(zard.name, "Charizard");
    assert_eq!(zard.set_id, "base1");
    assert_eq!(zard.supertype, "Pokémon");
    assert!(zard.hp_value() == Some(120));
    assert!(!zard.attacks.is_empty());
    assert_eq!(zard.rarity.as_deref(), Some("Rare Holo"));
    assert!(zard
        .images
        .as_ref()
        .and_then(|i| i.large.as_deref())
        .is_some());
    // Toutes les informations brutes sont conservées.
    assert!(!zard.extra.is_empty());
}

#[test]
fn filters_should_narrow_cards() {
    let Some(db) = tcg() else { return };
    let all = db.cards().len();

    let fire = CardFilter {
        types: vec!["Fire".into()],
        ..CardFilter::default()
    };
    let fire_count = db.filter_indices(&fire, CardSort::Api).len();
    assert!(fire_count > 0);
    assert!(fire_count < all);

    // Filtres combinés : Pikachu dans une extension « Base ».
    let pikachu = CardFilter {
        query: "Pikachu".into(),
        set_id: Some("base1".into()),
        ..CardFilter::default()
    };
    let rows = db.filter_indices(&pikachu, CardSort::Api);
    assert_eq!(rows.len(), 1);
    assert_eq!(db.cards()[rows[0]].name, "Pikachu");

    // Recherche par nom d'extension (insensible aux accents).
    let base = CardFilter {
        query: "Base".into(),
        ..CardFilter::default()
    };
    assert!(db.filter_indices(&base, CardSort::Api).len() > 100);
}

#[test]
fn hp_sort_should_rank_highest_first() {
    let Some(db) = tcg() else { return };
    let rows = db.filter_indices(&CardFilter::default(), CardSort::Hp);
    let hps: Vec<u16> = rows
        .iter()
        .take(10)
        .map(|&i| db.cards()[i].hp_value().unwrap_or(0))
        .collect();
    assert!(
        hps.windows(2).all(|w| w[0] >= w[1]),
        "les PV doivent être décroissants : {hps:?}"
    );
    assert!(
        hps[0] >= 300,
        "les cartes les plus robustes dépassent 300 PV : {hps:?}"
    );
}

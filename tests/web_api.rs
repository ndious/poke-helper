//! Tests de l'API JSON consommée par l'interface web.

use pokedex::db::Pokedex;
use pokedex::tcg::TcgDatabase;
use pokedex::web::{
    card_detail_json, cards_json, meta_json, pokemon_detail_json, pokemons_json, CardParams,
    PokemonParams, WebData,
};

fn data() -> WebData {
    let dex = Pokedex::load_embedded().expect("dataset embarqué");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    match TcgDatabase::load_dir(&dir) {
        Ok(tcg) => WebData {
            dex,
            tcg: Some(tcg),
        },
        Err(e) => {
            eprintln!("catalogue TCG absent ({e}) : partie cartes ignorée");
            WebData { dex, tcg: None }
        }
    }
}

fn dex_params() -> PokemonParams {
    PokemonParams {
        q: Some("pika".into()),
        ..PokemonParams::default()
    }
}

#[test]
fn pokemon_search_should_find_pikachu() {
    let data = data();
    let res = pokemons_json(&data, &dex_params());
    assert_eq!(res["total"], 1, "seule la forme par défaut de Pikachu");
    assert_eq!(res["items"][0]["id"], 25);
    assert_eq!(res["items"][0]["generation"], 1);
    assert_eq!(res["pages"], 1);
}

#[test]
fn empty_and_invalid_params_should_be_ignored() {
    let data = data();
    // Chaînes vides ou non numériques : aucun filtre ne doit s'appliquer.
    let params = PokemonParams {
        q: Some(String::new()),
        type_id: Some(String::new()),
        gen: Some("abc".into()),
        legendary: Some("false".into()),
        page: Some("0".into()),
        ..PokemonParams::default()
    };
    let res = pokemons_json(&data, &params);
    assert_eq!(res["total"], 1025);
    // Page 0 ramenée à 1.
    assert_eq!(res["page"], 1);
}

#[test]
fn filters_should_combine() {
    let data = data();
    let params = PokemonParams {
        type_id: Some("12".into()), // Plante
        gen: Some("1".into()),
        ..PokemonParams::default()
    };
    let res = pokemons_json(&data, &params);
    assert_eq!(res["total"], 14);
}

#[test]
fn sort_total_should_rank_arceus_first() {
    let data = data();
    let params = PokemonParams {
        sort: Some("total".into()),
        per_page: Some("1".into()),
        ..PokemonParams::default()
    };
    let res = pokemons_json(&data, &params);
    assert_eq!(res["items"][0]["id"], 493);
}

#[test]
fn detail_should_expose_generation_and_effectiveness() {
    let data = data();
    let p = pokemon_detail_json(&data, 6).expect("Dracaufeu existe");
    assert_eq!(p["generation"], 1);
    assert_eq!(p["name_fr"], "Dracaufeu");
    assert_eq!(p["types"][0], "Feu");
    // Roche ×4, immunité Sol.
    assert!(p["effectiveness"]["x4"]
        .as_array()
        .is_some_and(|a| a.contains(&"Roche".into())));
    assert!(p["effectiveness"]["immune"]
        .as_array()
        .is_some_and(|a| a.contains(&"Sol".into())));
    assert_eq!(p["stats"]["total"], 534);
    assert_eq!(pokemon_detail_json(&data, 99999), None);
}

fn cards_catalog() -> Option<WebData> {
    let data = data();
    if data.tcg.is_none() {
        eprintln!("catalogue TCG absent : tests cartes ignorés");
        return None;
    }
    Some(data)
}

#[test]
fn cards_should_be_searchable_by_french_name() {
    let Some(data) = cards_catalog() else { return };
    let params = CardParams {
        q: Some("dracaufeu".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    assert!(
        res["total"].as_u64().unwrap_or(0) > 0,
        "« dracaufeu » doit trouver les cartes de Charizard"
    );
    assert_eq!(res["items"][0]["name"], "Dracaufeu");
    assert_eq!(res["items"][0]["name_en"], "Charizard");
}

#[test]
fn card_detail_should_include_set_and_number() {
    let Some(data) = cards_catalog() else { return };
    let c = card_detail_json(&data, "base1-4").expect("Dracaufeu de la série de base");
    assert_eq!(c["name"], "Dracaufeu");
    assert_eq!(c["set"]["name"], "Base");
    assert_eq!(c["set"]["series"], "Base");
    assert_eq!(c["attacks"][0]["name"], "Fire Spin");
    assert_eq!(c["hp"], "120");
}

#[test]
fn meta_should_list_filter_options() {
    let data = data();
    let meta = meta_json(&data);
    assert_eq!(meta["types"].as_array().map(Vec::len), Some(18));
    assert_eq!(meta["sets"].as_array().map(Vec::len), Some(176));
    assert_eq!(meta["generations"].as_array().map(Vec::len), Some(9));
    if let Some(rarities) = meta["rarities"].as_array() {
        assert!(rarities.iter().any(|r| r == "Rare Holo"));
    }
}

#[test]
fn pokemon_number_range_should_filter() {
    let data = data();
    // Gen 1 : 151 Pokémon.
    let params = PokemonParams {
        min: Some("1".into()),
        max: Some("151".into()),
        ..PokemonParams::default()
    };
    assert_eq!(pokemons_json(&data, &params)["total"], 151);
    // Bornes incluses : une seule espèce.
    let params = PokemonParams {
        min: Some("25".into()),
        max: Some("25".into()),
        ..PokemonParams::default()
    };
    assert_eq!(pokemons_json(&data, &params)["total"], 1);
    // Valeur non numérique ignorée.
    let params = PokemonParams {
        min: Some("abc".into()),
        ..PokemonParams::default()
    };
    assert_eq!(pokemons_json(&data, &params)["total"], 1025);
}

#[test]
fn cards_should_filter_by_species_number_and_generation() {
    let Some(data) = cards_catalog() else { return };
    // Toutes les cartes de Dracaufeu (espèce n°6), toutes éditions.
    let params = CardParams {
        min: Some("6".into()),
        max: Some("6".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    assert!(
        res["total"].as_u64().unwrap_or(0) > 10,
        "Dracaufeu a beaucoup de cartes"
    );
    assert!(res["items"][0]["name_en"]
        .as_str()
        .unwrap()
        .starts_with("Charizard"));
    // Génération 1 : aucun numéro postérieur à 151 ne doit ressortir.
    let params = CardParams {
        gen: Some("1".into()),
        set_id: Some("base1".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    let total = res["total"].as_u64().unwrap_or(0);
    assert!(
        total > 50 && total < 102,
        "la base contient des Pokémon et des dresseurs : {total}"
    );
}

#[test]
fn cards_should_filter_by_hp_range() {
    let Some(data) = cards_catalog() else { return };
    // Monstres à 300 PV ou plus, toutes éditions confondues.
    let params = CardParams {
        hp_min: Some("300".into()),
        per_page: Some("10".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    for item in res["items"].as_array().into_iter().flatten() {
        let hp: u16 = item["hp"]
            .as_str()
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        assert!(hp >= 300, "toute carte retournée doit avoir 300 PV ou plus");
    }
    // Borne haute : les cartes à 40 PV ou moins.
    let params = CardParams {
        hp_max: Some("40".into()),
        per_page: Some("10".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    for item in res["items"].as_array().into_iter().flatten() {
        let hp: u16 = item["hp"]
            .as_str()
            .and_then(|h| h.parse().ok())
            .unwrap_or(0);
        assert!(hp <= 40);
    }
}

#[test]
fn cards_should_filter_by_release_year() {
    let Some(data) = cards_catalog() else { return };
    // La base (1999) doit être exclue d'une fenêtre récente.
    let params = CardParams {
        year_min: Some("2023".into()),
        year_max: Some("2023".into()),
        per_page: Some("50".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    assert!(
        res["total"].as_u64().unwrap_or(0) > 100,
        "de nombreux sets sont sortis en 2023"
    );
    for item in res["items"].as_array().into_iter().flatten() {
        assert_eq!(
            item["year"], "2023",
            "seules des cartes sorties en 2023 doivent apparaître"
        );
    }
    // Filtre année + espèce : Dracaufeu dans les sets 2023.
    let params = CardParams {
        min: Some("6".into()),
        max: Some("6".into()),
        year_min: Some("2023".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    assert!(
        res["total"].as_u64().unwrap_or(0) > 0,
        "Dracaufeu a des cartes sorties en 2023"
    );
    // Valeur non numérique ignorée.
    let params = CardParams {
        year_min: Some("hier".into()),
        ..CardParams::default()
    };
    assert_eq!(cards_json(&data, &params)["total"], 20635);
}

#[test]
fn cards_should_sort_by_pokedex_number() {
    let Some(data) = cards_catalog() else { return };
    // Espèces 24-25, tri par numéro décroissant : Pikachu (25) en tête.
    let params = CardParams {
        min: Some("24".into()),
        max: Some("25".into()),
        sort: Some("number".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    assert!(res["items"][0]["name_en"]
        .as_str()
        .unwrap_or("")
        .starts_with("Pikachu"));
}

#[test]
fn cards_should_filter_by_evolution_stage() {
    let Some(data) = cards_catalog() else { return };
    // Dracaufeu (espèce 6) de niveau 2 uniquement.
    let params = CardParams {
        min: Some("6".into()),
        max: Some("6".into()),
        stage: Some("stage2".into()),
        per_page: Some("50".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    for item in res["items"].as_array().into_iter().flatten() {
        assert!(
            item["subtypes"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|s| s == "Stage 2"),
            "seules des cartes de niveau 2 doivent apparaître"
        );
    }
    assert!(res["total"].as_u64().unwrap_or(0) > 0);
    // Un sous-type inexistant ne matche aucune carte (match exact).
    let params = CardParams {
        stage: Some("megamax".into()),
        min: Some("6".into()),
        max: Some("6".into()),
        ..CardParams::default()
    };
    let none = cards_json(&data, &params)["total"].as_u64().unwrap_or(0);
    assert_eq!(none, 0, "aucune carte ne porte ce sous-type");
}

#[test]
fn card_items_should_expose_year_for_species_table() {
    let Some(data) = cards_catalog() else { return };
    let params = CardParams {
        set_id: Some("base1".into()),
        per_page: Some("10".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    for item in res["items"].as_array().into_iter().flatten() {
        assert_eq!(item["year"], "1999", "la base est sortie en 1999");
    }
}

#[test]
fn pokemons_should_honor_pinned_ids_list() {
    let data = data();
    // La sélection épinglée ignore les autres filtres.
    let params = PokemonParams {
        ids: Some("25,6,150,99999".into()),
        q: Some("bulbizarre".into()),
        type_id: Some("10".into()),
        ..PokemonParams::default()
    };
    let res = pokemons_json(&data, &params);
    assert_eq!(
        res["total"], 3,
        "99999 n'existe pas, les 3 autres reviennent"
    );
    let ids: Vec<u32> = res["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i["id"].as_u64().map(|v| v as u32))
        .collect();
    assert_eq!(ids, vec![25, 6, 150]);
    // Chaque fiche est complète (types, génération).
    assert_eq!(res["items"][1]["name_fr"], "Dracaufeu");
    // Paramètre vide ou invalide : mode désactivé.
    let params = PokemonParams {
        ids: Some(String::new()),
        ..PokemonParams::default()
    };
    assert_eq!(pokemons_json(&data, &params)["total"], 1025);
    let params = PokemonParams {
        ids: Some(",,,".into()),
        ..PokemonParams::default()
    };
    assert_eq!(pokemons_json(&data, &params)["total"], 1025);
}

#[test]
fn pinned_ids_mode_should_include_evolution_info_for_table_view() {
    let data = data();
    let params = PokemonParams {
        ids: Some("1,6,25".into()),
        ..PokemonParams::default()
    };
    let res = pokemons_json(&data, &params);
    let items = res["items"].as_array().expect("3 épinglés");
    assert_eq!(items.len(), 3);
    let bulbi = &items[0];
    assert_eq!(bulbi["stage"], "Base");
    assert_eq!(
        bulbi["evolutions"],
        serde_json::json!(["Herbizarre"]),
        "évolutions directes uniquement"
    );
    assert_eq!(bulbi["line"], "Bulbizarre → Herbizarre → Florizarre");
    assert_eq!(
        items[1]["stage"], "Niveau 2",
        "Dracaufeu : deux pré-évolutions"
    );
    assert_eq!(items[1]["evolutions"], serde_json::json!([]));
    // Pikachu évolue depuis Pichu : stade Niveau 1.
    assert_eq!(items[2]["stage"], "Niveau 1");
}

#[test]
fn evolution_data_should_be_consistent() {
    let data = data();
    // Évoli : 8 évolutions, lignée de 9 membres.
    assert_eq!(data.dex.evolutions(133).len(), 8);
    assert_eq!(data.dex.evolution_line(133).len(), 9);
    assert!(data.dex.has_evolution(133));
    assert_eq!(data.dex.stage_label(133), "Base");
    // Mewtwo : aucune évolution, lignée réduite à lui-même.
    assert!(!data.dex.has_evolution(150));
    assert_eq!(data.dex.evolution_line(150).len(), 1);
    assert_eq!(data.dex.evolution_line(150)[0].name_fr, "Mewtwo");
    // Dracaufeu : dernier maillon de sa lignée de 3.
    let line = data.dex.evolution_line(6);
    let names: Vec<&str> = line.iter().map(|p| p.name_fr.as_str()).collect();
    assert_eq!(names, vec!["Salamèche", "Reptincel", "Dracaufeu"]);
}

#[test]
fn cards_should_filter_by_multiple_types() {
    let Some(data) = cards_catalog() else { return };
    // Un type : Fire.
    let fire = cards_json(
        &data,
        &CardParams {
            types: Some("Fire".into()),
            per_page: Some("1".into()),
            ..CardParams::default()
        },
    )["total"]
        .as_u64()
        .unwrap_or(0);
    let water = cards_json(
        &data,
        &CardParams {
            types: Some("Water".into()),
            per_page: Some("1".into()),
            ..CardParams::default()
        },
    )["total"]
        .as_u64()
        .unwrap_or(0);
    // Deux types : union (au moins un des deux).
    let both = cards_json(
        &data,
        &CardParams {
            types: Some("Fire,Water".into()),
            per_page: Some("1".into()),
            ..CardParams::default()
        },
    )["total"]
        .as_u64()
        .unwrap_or(0);
    assert!(fire > 0 && water > 0);
    // Union : au moins chacun des filtres simples, sans double-compter
    // les cartes double-type (Feu ET Eau).
    assert!(
        both >= fire && both >= water,
        "l'union contient chaque filtre simple"
    );
    assert!(both <= fire + water, "l'union ne dépasse pas la somme");
    // Paramètre vide ou espaces : aucun filtre.
    let res = cards_json(
        &data,
        &CardParams {
            types: Some(", ,".into()),
            per_page: Some("1".into()),
            ..CardParams::default()
        },
    );
    assert_eq!(res["total"], 20635);
}

#[test]
fn stage_filter_should_expose_all_catalog_subtypes() {
    let Some(data) = cards_catalog() else { return };
    // Le méta liste tous les sous-types, VMAX en tête de file des mécaniques.
    let meta = meta_json(&data);
    let stages = meta["stages"].as_array().expect("liste des stades");
    assert!(stages.len() > 25, "le catalogue contient de nombreux sous-types");
    let vmax = stages
        .iter()
        .find(|s| s["key"] == "VMAX")
        .expect("VMAX présent");
    assert_eq!(vmax["fr"], "Gigamax");
    assert!(vmax["count"].as_u64().unwrap_or(0) > 100);

    // Filtre direct par sous-type : toutes les cartes Gigamax.
    let params = CardParams {
        stage: Some("VMAX".into()),
        per_page: Some("20".into()),
        ..CardParams::default()
    };
    let res = cards_json(&data, &params);
    let total = res["total"].as_u64().unwrap_or(0);
    assert_eq!(total, vmax["count"].as_u64().unwrap_or(0));
    for item in res["items"].as_array().into_iter().flatten() {
        assert!(
            item["subtypes"].as_array().into_iter().flatten().any(|s| s == "VMAX"),
            "seules des cartes VMAX doivent apparaître"
        );
    }

    // Autres mécaniques connues, toutes presentes dans le méta.
    for key in ["VSTAR", "GX", "V", "ex", "EX", "MEGA", "Tera", "Baby"] {
        assert!(
            stages.iter().any(|s| s["key"] == key),
            "le sous-type {key} devrait être listé"
        );
    }
}

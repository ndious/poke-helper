//! Serveur web de la base de données (interface à la souris).
//!
//! `cargo run` depuis la racine, puis http://127.0.0.1:8080
//! (port modifiable via `PORT=…`).

use std::path::Path;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use serde_json::Value;

use pokedex::db::Pokedex;
use pokedex::tcg::TcgDatabase;
use pokedex::web::{
    card_detail_json, cards_json, meta_json, pokemon_detail_json, pokemons_json, CardParams,
    PokemonParams, WebData,
};

type Shared = Arc<WebData>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dex = Pokedex::load_embedded()
        .map_err(|e| anyhow::anyhow!("Pokédex embarqué illisible : {e}"))?;
    let tcg = match TcgDatabase::load_dir(Path::new("data")) {
        Ok(db) => Some(db),
        Err(e) => {
            eprintln!(
                "Note : cartes TCG non chargées ({e}).\n\
                 Lancez « cargo run --release --bin pokedex-tcg-fetch » pour les télécharger."
            );
            None
        }
    };
    let state: Shared = Arc::new(WebData { dex, tcg });
    let cards = state.tcg.as_ref().map(|t| t.cards().len()).unwrap_or(0);
    eprintln!(
        "{} Pokémon, {cards} cartes TCG chargés",
        state.dex.pokemons().len()
    );

    let app = Router::new()
        .route("/", get(index))
        .route("/api/pokemons", get(list_pokemons))
        .route("/api/pokemons/{id}", get(get_pokemon))
        .route("/api/cards", get(list_cards))
        .route("/api/cards/{id}", get(get_card))
        .route("/api/meta", get(get_meta))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("Interface web : http://127.0.0.1:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}

async fn list_pokemons(
    State(state): State<Shared>,
    Query(params): Query<PokemonParams>,
) -> Json<Value> {
    Json(pokemons_json(&state, &params))
}

async fn get_pokemon(
    State(state): State<Shared>,
    AxumPath(id): AxumPath<u32>,
) -> impl IntoResponse {
    match pokemon_detail_json(&state, id) {
        Some(v) => Json(v).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn list_cards(State(state): State<Shared>, Query(params): Query<CardParams>) -> Json<Value> {
    Json(cards_json(&state, &params))
}

async fn get_card(
    State(state): State<Shared>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    match card_detail_json(&state, &id) {
        Some(v) => Json(v).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_meta(State(state): State<Shared>) -> Json<Value> {
    Json(meta_json(&state))
}

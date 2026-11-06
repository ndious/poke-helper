//! Tests de la logique applicative TUI (sans terminal réel) et du rendu
//! (via le TestBackend de ratatui).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};

use pokedex::app::{App, Mode, PopupKind, Tab};
use pokedex::db::Pokedex;
use pokedex::tcg::TcgDatabase;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn char_key(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

fn app() -> App {
    let dex = Pokedex::load_embedded().expect("dataset embarqué");
    let tcg = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    App::new(dex, TcgDatabase::load_dir(&tcg).ok())
}

/// Le tampon complet du rendu sous forme de texte.
fn rendered_text(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("backend de test");
    terminal
        .draw(|f| pokedex::ui::draw(f, app))
        .expect("rendu sans erreur");
    let buf = terminal.backend().buffer();
    let mut text = String::with_capacity((buf.area.width * buf.area.height) as usize);
    for cell in buf.content() {
        text.push_str(cell.symbol());
    }
    text
}

fn type_keys(app: &mut App, s: &str) {
    for c in s.chars() {
        app.on_key(char_key(c));
    }
}

#[test]
fn app_should_start_with_all_pokemons_and_first_selected() {
    let app = app();
    assert_eq!(app.dex_rows.len(), 1025);
    assert_eq!(app.dex_state.selected(), Some(0));
    assert_eq!(app.selected_pokemon().map(|p| p.id), Some(1));
}

#[test]
fn search_should_filter_dex_rows_live() {
    let mut app = app();
    app.on_key(char_key('/'));
    assert_eq!(app.mode, Mode::Search);

    type_keys(&mut app, "pika");
    // Seule la forme par défaut de Pikachu est dans le dataset.
    assert_eq!(app.dex_rows, vec![25]);

    // La sélection repart au début après filtrage.
    assert_eq!(app.dex_state.selected(), Some(0));

    // Échap quitte le mode saisie, x réinitialise tout.
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal);
    app.on_key(char_key('x'));
    assert_eq!(app.dex_rows.len(), 1025);
}

#[test]
fn type_popup_should_filter_pokedex() {
    let mut app = app();
    app.on_key(char_key('t'));
    assert_eq!(app.popup.as_ref().map(|p| p.kind), Some(PopupKind::DexType));

    // Filtrage local du popup : « fe » ne laisse que Feu et Fée.
    type_keys(&mut app, "fe");
    let items = app.popup_items();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|i| i.label == "Feu"));

    // Sélection de Feu (premier élément).
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.dex_filters.type_id, Some(10));
    let expected = app
        .dex
        .pokemons()
        .iter()
        .filter(|p| p.types.contains(&10))
        .count();
    assert_eq!(app.dex_rows.len(), expected);

    // Rouvrir et re-sélectionner la même valeur désactive le filtre.
    app.on_key(char_key('t'));
    type_keys(&mut app, "feu");
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.dex_filters.type_id, None);
    assert_eq!(app.dex_rows.len(), 1025);
}

#[test]
fn generation_key_should_cycle_through_generations() {
    let mut app = app();
    app.on_key(char_key('g'));
    assert_eq!(app.dex_filters.generation, Some(1));
    let gen1 = app.dex_rows.len();
    assert!(app.dex_rows.iter().all(|id| {
        app.dex
            .get(*id)
            .map(|p| p.generation == 1)
            .unwrap_or_default()
    }));
    for _ in 2..=10 {
        app.on_key(char_key('g'));
    }
    // 10 pressions au total : aucune→1→2→…→9→aucune.
    assert_eq!(app.dex_filters.generation, None);
    assert_eq!(app.dex_rows.len(), 1025);
    assert!(gen1 < 1025);
}

#[test]
fn legendary_and_mythical_toggles_should_filter() {
    let mut app = app();
    app.on_key(char_key('l'));
    assert!(app.dex_filters.legendary_only);
    let legendary = app.dex_rows.len();
    let expected = app.dex.pokemons().iter().filter(|p| p.is_legendary).count();
    assert_eq!(legendary, expected);

    app.on_key(char_key('x'));
    app.on_key(char_key('m'));
    let expected_mythical = app.dex.pokemons().iter().filter(|p| p.is_mythical).count();
    assert_eq!(app.dex_rows.len(), expected_mythical);
    assert!(expected_mythical < legendary);
}

#[test]
fn sort_key_should_cycle_orders() {
    let mut app = app();
    app.on_key(char_key('s'));
    assert_eq!(app.dex_filters.sort.label(), "Nom");
    app.on_key(char_key('s'));
    assert_eq!(app.dex_filters.sort.label(), "Total");
    // Tri par total : Arceus (720) en tête.
    let first = app.dex_rows[0];
    assert_eq!(first, 493);
}

#[test]
fn navigation_should_move_selection_and_scroll() {
    let mut app = app();
    app.on_key(key(KeyCode::Down));
    app.on_key(key(KeyCode::Down));
    assert_eq!(app.dex_state.selected(), Some(2));
    assert_eq!(app.selected_pokemon().map(|p| p.id), Some(3));
    app.on_key(key(KeyCode::Home));
    assert_eq!(app.dex_state.selected(), Some(0));
    app.on_key(key(KeyCode::PageDown));
    assert_eq!(app.dex_state.selected(), Some(20));
    // Le défilement du détail bouge sans casser la sélection.
    app.on_key(char_key('.'));
    assert_eq!(app.detail_scroll, 2);
    assert_eq!(app.dex_state.selected(), Some(20));
}

#[test]
fn quit_should_set_should_quit() {
    let mut app = app();
    app.on_key(char_key('q'));
    assert!(app.should_quit);
}

#[test]
fn render_should_show_tabs_list_and_selected_details() {
    let app = app();
    let text = rendered_text(&app);
    for expected in [
        "Pokédex",
        "Cartes TCG",
        "Bulbizarre",
        "Plante / Poison",
        "résultat",
    ] {
        assert!(
            text.contains(expected),
            "le rendu devrait contenir « {expected} »"
        );
    }
}

#[test]
fn render_should_show_tcg_tab_when_data_present() {
    let mut app = app();
    if app.tcg.is_none() {
        eprintln!("données TCG absentes : test ignoré");
        return;
    }
    app.on_key(key(KeyCode::Tab));
    assert_eq!(app.tab, Tab::Cards);
    assert!(!app.card_rows.is_empty());
    let text = rendered_text(&app);
    assert!(
        text.contains("rareté"),
        "le footer de l'onglet cartes doit lister « rareté »"
    );
}

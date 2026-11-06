//! Pokédex interactif — TUI plein écran (ratatui/crossterm).
//!
//! Onglets : [Pokédex] les 1025 Pokémon · [Cartes TCG] le catalogue complet
//! des cartes physiques et digitales (généré par `pokedex-tcg-fetch`).

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::Terminal;

use pokedex::app::App;
use pokedex::db::Pokedex;
use pokedex::tcg::TcgDatabase;
use pokedex::ui;

fn main() -> anyhow::Result<()> {
    let dex = Pokedex::load_embedded()
        .map_err(|e| anyhow::anyhow!("Pokédex embarqué illisible : {e}"))?;

    let tcg = match TcgDatabase::load_dir(std::path::Path::new("data")) {
        Ok(db) => Some(db),
        Err(e) => {
            eprintln!("Note : cartes TCG non chargées ({e})");
            eprintln!(
                "Lancez « cargo run --release --bin pokedex-tcg-fetch » pour les télécharger."
            );
            None
        }
    };

    let mut terminal =
        ratatui::try_init().map_err(|e| anyhow::anyhow!("terminal non interactif : {e}"))?;
    let result = run(&mut terminal, App::new(dex, tcg));
    ratatui::restore();
    result
}

fn run(
    terminal: &mut Terminal<impl ratatui::backend::Backend>,
    mut app: App,
) -> anyhow::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui::draw(f, &app))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
            _ => {}
        }
    }
    Ok(())
}

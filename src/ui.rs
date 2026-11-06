//! Rendu de l'interface TUI (ratatui) : onglets, filtres, listes, détail.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{App, Mode, Tab};
use crate::render as text_render;
use crate::tcg::{energy_fr, TcgCard, TcgSet};

/// Deux spans : la touche en gras cyan, puis le libellé.
fn key_spans(key: &str, label: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {label} · ")),
    ]
}

/// Aide affichée en pied d'écran.
fn footer_line(app: &App) -> Line<'static> {
    let mut keys: Vec<(&str, &str)> = match app.tab {
        Tab::Pokedex => vec![
            ("↑↓", "naviguer"),
            ("/", "recherche"),
            ("t", "type"),
            ("g", "génération"),
            ("l", "légend."),
            ("m", "mythique"),
        ],
        Tab::Cards => vec![
            ("↑↓", "naviguer"),
            ("/", "recherche"),
            ("t", "type"),
            ("r", "rareté"),
            ("e", "extension"),
        ],
    };
    keys.extend_from_slice(&[
        ("s", "tri"),
        ("Tab", "onglet"),
        ("x", "reset"),
        ("i", "zoom"),
        (",/.", "détail"),
        ("q", "quitter"),
    ]);
    Line::from(
        keys.into_iter()
            .flat_map(|(k, l)| key_spans(k, l))
            .collect::<Vec<_>>(),
    )
}

/// Résumé des filtres actifs, sous les onglets.
fn filters_line(app: &App) -> Line<'static> {
    let mut spans = Vec::new();
    match app.tab {
        Tab::Pokedex => {
            let f = &app.dex_filters;
            let query = if f.query.is_empty() {
                "—".to_string()
            } else {
                f.query.clone()
            };
            spans.push(Span::raw(format!("Recherche : {query}")));
            if let Some(t) = f.type_id {
                spans.push(Span::raw(format!("  Type : {}", app.dex.type_label(t))));
            }
            if let Some(g) = f.generation {
                spans.push(Span::raw(format!("  Génération : {g}")));
            }
            if f.legendary_only {
                spans.push(Span::raw("  Légendaires"));
            }
            if f.mythical_only {
                spans.push(Span::raw("  Mythiques"));
            }
            spans.push(Span::raw(format!("  Tri : {}", f.sort.label())));
        }
        Tab::Cards => {
            let f = &app.card_filters;
            let query = if f.query.is_empty() {
                "—".to_string()
            } else {
                f.query.clone()
            };
            spans.push(Span::raw(format!("Recherche : {query}")));
            if let Some(t) = &f.type_ {
                spans.push(Span::raw(format!("  Type : {}", energy_fr(t))));
            }
            if let Some(r) = &f.rarity {
                spans.push(Span::raw(format!("  Rareté : {r}")));
            }
            if let Some(s) = &f.set_id {
                let name = app
                    .tcg()
                    .and_then(|db| db.set(s))
                    .map(|set| set.name.as_str())
                    .unwrap_or(s.as_str());
                spans.push(Span::raw(format!("  Extension : {name}")));
            }
            spans.push(Span::raw(format!("  Tri : {}", f.sort.label())));
        }
    }
    if app.mode == Mode::Search {
        spans.push(Span::styled(
            " [saisie en cours…]",
            Style::new().fg(Color::Yellow),
        ));
    }
    Line::from(spans)
}

/// Ligne de liste pour un Pokémon.
fn dex_list_item(app: &App, id: u32) -> ListItem<'static> {
    let Some(p) = app.dex.get(id) else {
        return ListItem::new("");
    };
    let types: Vec<&str> = p.types.iter().map(|&t| app.dex.type_label(t)).collect();
    let marker = if p.is_legendary {
        "★"
    } else if p.is_mythical {
        "☆"
    } else {
        " "
    };
    ListItem::new(format!(
        "#{id:<4}{marker} {:<12} {:<15} {:>3}",
        p.name_fr,
        types.join("/"),
        p.stats.total()
    ))
}

/// Ligne de liste pour une carte TCG.
fn card_list_item(app: &App, index: usize) -> ListItem<'static> {
    let Some(tcg) = app.tcg() else {
        return ListItem::new("");
    };
    let Some(c) = tcg.cards().get(index) else {
        return ListItem::new("");
    };
    let set = tcg.set(&c.set_id);
    let set_name = set.map(|s| s.name.as_str()).unwrap_or(&c.set_id);
    let type_label = c
        .types
        .first()
        .map(|t| energy_fr(t).to_string())
        .unwrap_or_else(|| c.supertype.clone());
    ListItem::new(format!(
        "{:<20} {:<28} {:<10} {:<4}",
        truncate(c.display_name(), 20),
        set_name,
        truncate(&type_label, 10),
        c.number
    ))
}

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        s.chars().take(width.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Détail d'une carte TCG (panneau de droite).
fn card_detail_lines(app: &App, c: &TcgCard) -> Vec<Line<'static>> {
    let tcg = app.tcg();
    let mut lines = Vec::new();
    let title = format!("{} · {}", c.display_name(), c.supertype);
    lines.push(Line::styled(
        title,
        Style::new().add_modifier(Modifier::BOLD).fg(Color::White),
    ));
    if c.name_fr.is_some() {
        lines.push(Line::styled(
            format!("({})", c.name),
            Style::new().fg(Color::DarkGray),
        ));
    }
    if !c.subtypes.is_empty() {
        lines.push(Line::raw(format!("Stade : {}", c.subtypes.join(", "))));
    }
    if let Some(set) = tcg.and_then(|db| db.set(&c.set_id)) {
        lines.push(Line::raw(set_line(set, &c.number)));
    }
    if let Some(hp) = &c.hp {
        lines.push(Line::raw(format!("PV : {hp}")));
    }
    if !c.types.is_empty() {
        let types: Vec<&str> = c.types.iter().map(|t| energy_fr(t)).collect();
        lines.push(Line::raw(format!("Type(s) : {}", types.join(", "))));
    }
    if let Some(e) = &c.evolves_from {
        lines.push(Line::raw(format!("Évolue de : {e}")));
    }
    if let Some(r) = &c.rarity {
        lines.push(Line::raw(format!("Rareté : {r}")));
    }
    if let Some(a) = &c.artist {
        lines.push(Line::raw(format!("Illustrateur : {a}")));
    }
    if !c.attacks.is_empty() {
        lines.push(Line::styled(
            "Attaques",
            Style::new().add_modifier(Modifier::BOLD),
        ));
        for atk in &c.attacks {
            let cost: Vec<&str> = atk.cost.iter().map(|t| energy_fr(t)).collect();
            let damage = atk.damage.clone().unwrap_or_default();
            lines.push(Line::raw(format!(
                "  {} [{}] : {damage}",
                atk.name,
                cost.join("+")
            )));
            if let Some(t) = &atk.text {
                for wrap in wrap_text(t, 60, "    ") {
                    lines.push(Line::raw(wrap));
                }
            }
        }
    }
    if !c.weaknesses.is_empty() {
        let w: Vec<String> = c
            .weaknesses
            .iter()
            .map(|x| format!("{} {}", energy_fr(&x.ty), x.value))
            .collect();
        lines.push(Line::raw(format!("Faiblesse : {}", w.join(", "))));
    }
    if !c.resistances.is_empty() {
        let r: Vec<String> = c
            .resistances
            .iter()
            .map(|x| format!("{} {}", energy_fr(&x.ty), x.value))
            .collect();
        lines.push(Line::raw(format!("Résistance : {}", r.join(", "))));
    }
    if let Some(rc) = c.converted_retreat_cost {
        lines.push(Line::raw(format!("Repli : {rc}")));
    }
    let legalities: Vec<String> = c
        .legalities
        .iter()
        .map(|(fmt, status)| format!("{fmt} : {status}"))
        .collect();
    if !legalities.is_empty() {
        lines.push(Line::raw(format!("Légalités : {}", legalities.join(", "))));
    }
    if let Some(price) = c.market_price() {
        lines.push(Line::styled(
            format!("Prix marché (TCGPlayer) : {price:.2} $"),
            Style::new().fg(Color::Green),
        ));
    }
    if let Some(f) = &c.flavor_text {
        for w in wrap_text(f, 60, "") {
            lines.push(Line::raw(w));
        }
    }
    if let Some(img) = c.images.as_ref().and_then(|i| i.small.as_deref()) {
        lines.push(Line::styled(
            img.to_string(),
            Style::new().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::styled(
        format!("ID : {}", c.id),
        Style::new().fg(Color::DarkGray),
    ));
    lines
}

fn set_line(s: &TcgSet, card_number: &str) -> String {
    format!(
        "Extension : {} — {} ({} cartes, {}/{})",
        s.series,
        s.name,
        s.release_date.replace('/', "-"),
        card_number,
        s.total
    )
}

/// Découpe un texte long en lignes d'au plus `width` caractères.
fn wrap_text(text: &str, width: usize, indent: &str) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut current = String::new();
        for word in para.split(' ') {
            if current.chars().count() + word.chars().count() + 1 > width {
                out.push(format!("{indent}{current}"));
                current.clear();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        out.push(format!("{indent}{current}"));
    }
    out
}

/// Rectangle centré (pour les popups).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [h] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(ratatui::layout::Flex::Center)
        .areas(area);
    let [r] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(ratatui::layout::Flex::Center)
        .areas(h);
    r
}

/// Rendu complet d'une frame.
pub fn draw(f: &mut Frame, app: &App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(f.area());

    let titles = [Tab::Pokedex, Tab::Cards]
        .iter()
        .map(|t| t.label().to_string())
        .collect::<Vec<_>>();
    f.render_widget(
        Tabs::new(titles)
            .select(app.tab as usize)
            .highlight_style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        header,
    );
    f.render_widget(
        Paragraph::new(filters_line(app)),
        Rect {
            y: header.y + 2,
            height: 2,
            ..header
        },
    );

    let split: [u16; 2] = if app.detail_zoom { [30, 70] } else { [55, 45] };
    let [list_area, detail_area] = Layout::horizontal([
        Constraint::Percentage(split[0]),
        Constraint::Percentage(split[1]),
    ])
    .areas(body);

    let list_title = format!(" {} résultat(s) ", app.visible_len());
    let block = Block::new()
        .borders(Borders::ALL)
        .title(list_title)
        .border_style(Style::new().fg(Color::Blue));
    f.render_widget(block, list_area);
    let inner = Rect {
        x: list_area.x + 1,
        y: list_area.y + 1,
        width: list_area.width.saturating_sub(2),
        height: list_area.height.saturating_sub(2),
    };

    match app.tab {
        Tab::Pokedex => {
            let items: Vec<ListItem> = app
                .dex_rows
                .iter()
                .map(|&id| dex_list_item(app, id))
                .collect();
            let mut state = app.dex_state.clone();
            f.render_stateful_widget(
                List::new(items).highlight_style(Style::new().bg(Color::DarkGray)),
                inner,
                &mut state,
            );
        }
        Tab::Cards => {
            if app.tcg.is_none() {
                f.render_widget(
                    Paragraph::new(
                        "Aucune donnée TCG.\n\nGénérez le catalogue avec :\n  cargo run --release --bin pokedex-tcg-fetch",
                    )
                    .block(Block::new().borders(Borders::ALL)),
                    inner,
                );
            } else {
                let items: Vec<ListItem> = app
                    .card_rows
                    .iter()
                    .map(|&i| card_list_item(app, i))
                    .collect();
                let mut state = app.card_state.clone();
                f.render_stateful_widget(
                    List::new(items).highlight_style(Style::new().bg(Color::DarkGray)),
                    inner,
                    &mut state,
                );
            }
        }
    }

    // Panneau de détail.
    let detail_block = Block::new()
        .borders(Borders::ALL)
        .title(" Détail ")
        .border_style(Style::new().fg(Color::Cyan));
    let detail_text = match app.tab {
        Tab::Pokedex => app
            .selected_pokemon()
            .map(|p| {
                let s = text_render::card(&app.dex, p);
                Paragraph::new(s)
                    .wrap(Wrap { trim: false })
                    .scroll((app.detail_scroll, 0))
            })
            .unwrap_or_else(|| Paragraph::new("Sélection vide")),
        Tab::Cards => app
            .selected_card()
            .map(|c| {
                Paragraph::new(card_detail_lines(app, c))
                    .wrap(Wrap { trim: false })
                    .scroll((app.detail_scroll, 0))
            })
            .unwrap_or_else(|| Paragraph::new("Sélection vide")),
    };
    f.render_widget(detail_block, detail_area);
    let detail_inner = Rect {
        x: detail_area.x + 1,
        y: detail_area.y + 1,
        width: detail_area.width.saturating_sub(2),
        height: detail_area.height.saturating_sub(2),
    };
    f.render_widget(detail_text, detail_inner);

    f.render_widget(Paragraph::new(footer_line(app)), footer);

    // Popup au-dessus de tout.
    if let Some(popup) = app.popup.as_ref() {
        let items = app.popup_items();
        let height = 14.min(f.area().height.saturating_sub(2));
        let rect = centered_rect(46, height, f.area());
        f.render_widget(Clear, rect);
        let block = Block::new()
            .borders(Borders::ALL)
            .title(format!(
                " {} — tapez pour filtrer, Entrée sélectionne, Échap ferme ",
                popup.kind.title()
            ))
            .border_style(Style::new().fg(Color::Magenta));
        f.render_widget(block, rect);
        let popup_area = Rect {
            x: rect.x + 1,
            y: rect.y + 1,
            width: rect.width.saturating_sub(2),
            height: rect.height.saturating_sub(2),
        };
        let query_line = Line::from(vec![Span::styled(
            format!("Recherche : {}", popup.query),
            Style::new().fg(Color::Yellow),
        )]);
        let list_height = popup_area.height.saturating_sub(2);
        f.render_widget(Paragraph::new(query_line), popup_area);
        let list_items: Vec<ListItem> = items
            .iter()
            .map(|i| ListItem::new(i.label.clone()))
            .collect();
        let mut state = ListState::default();
        if popup.selection < items.len() {
            state.select(Some(popup.selection));
        }
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::new().bg(Color::DarkGray)),
            Rect {
                y: popup_area.y + 1,
                height: list_height,
                ..popup_area
            },
            &mut state,
        );
    }
}

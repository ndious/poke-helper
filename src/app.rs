//! État de l'application TUI : onglets, filtres, navigation.
//!
//! La logique est volontairement séparée de l'affichage (`ui.rs`) :
//! `App::on_key` est testable sans terminal réel.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;

use crate::db::{Filter, Pokedex, SortKey};
use crate::tcg::{match_label, CardFilter, CardSort, TcgDatabase};

/// Onglets principaux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Pokedex,
    Cards,
}

impl Tab {
    pub fn toggle(self) -> Tab {
        match self {
            Tab::Pokedex => Tab::Cards,
            Tab::Cards => Tab::Pokedex,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Pokedex => "Pokédex",
            Tab::Cards => "Cartes TCG",
        }
    }
}

/// Mode de saisie : navigation normale ou édition de la recherche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
}

/// Popups de sélection d'un critère de filtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// Type du Pokédex (18 types FR).
    DexType,
    /// Type d'énergie d'une carte TCG.
    CardType,
    Rarity,
    /// Extension TCG.
    Set,
}

impl PopupKind {
    pub fn title(self) -> &'static str {
        match self {
            PopupKind::DexType => "Type",
            PopupKind::CardType => "Type d'énergie",
            PopupKind::Rarity => "Rareté",
            PopupKind::Set => "Extension",
        }
    }
}

/// État d'un popup ouvert : recherche locale + sélection courante.
#[derive(Debug, Clone)]
pub struct PopupState {
    pub kind: PopupKind,
    pub query: String,
    pub selection: usize,
}

impl PopupState {
    fn new(kind: PopupKind) -> PopupState {
        PopupState {
            kind,
            query: String::new(),
            selection: 0,
        }
    }
}

/// Ordres de tri disponibles pour le Pokédex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexSort {
    Id,
    Name,
    Total,
    Hp,
    Speed,
}

impl DexSort {
    pub fn cycle(self) -> DexSort {
        match self {
            DexSort::Id => DexSort::Name,
            DexSort::Name => DexSort::Total,
            DexSort::Total => DexSort::Hp,
            DexSort::Hp => DexSort::Speed,
            DexSort::Speed => DexSort::Id,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DexSort::Id => "N°",
            DexSort::Name => "Nom",
            DexSort::Total => "Total",
            DexSort::Hp => "PV",
            DexSort::Speed => "Vitesse",
        }
    }

    fn to_sort_key(self) -> SortKey {
        match self {
            DexSort::Id => SortKey::Id,
            DexSort::Name => SortKey::Name,
            _ => SortKey::Stat(self.stat_field()),
        }
    }

    fn stat_field(self) -> crate::db::StatField {
        match self {
            DexSort::Total => crate::db::StatField::Total,
            DexSort::Hp => crate::db::StatField::Hp,
            _ => crate::db::StatField::Speed,
        }
    }
}

/// Filtres de l'onglet Pokédex.
#[derive(Debug, Clone)]
pub struct DexFilters {
    pub query: String,
    pub type_id: Option<u32>,
    pub generation: Option<u8>,
    pub legendary_only: bool,
    pub mythical_only: bool,
    pub sort: DexSort,
}

impl Default for DexFilters {
    fn default() -> Self {
        DexFilters {
            query: String::new(),
            type_id: None,
            generation: None,
            legendary_only: false,
            mythical_only: false,
            sort: DexSort::Id,
        }
    }
}

/// Filtres de l'onglet Cartes TCG.
#[derive(Debug, Clone)]
pub struct CardFilters {
    pub query: String,
    pub type_: Option<String>,
    pub rarity: Option<String>,
    pub set_id: Option<String>,
    pub sort: CardSort,
}

impl Default for CardFilters {
    fn default() -> Self {
        CardFilters {
            query: String::new(),
            type_: None,
            rarity: None,
            set_id: None,
            sort: CardSort::Api,
        }
    }
}

/// État global de l'application.
pub struct App {
    pub dex: Pokedex,
    pub tcg: Option<TcgDatabase>,
    pub tab: Tab,
    pub mode: Mode,
    pub dex_filters: DexFilters,
    pub card_filters: CardFilters,
    /// Identifiants des Pokémon visibles après filtrage.
    pub dex_rows: Vec<u32>,
    /// Indices (dans TcgDatabase::cards) des cartes visibles.
    pub card_rows: Vec<usize>,
    pub dex_state: ListState,
    pub card_state: ListState,
    pub popup: Option<PopupState>,
    /// Défilement du panneau de détail.
    pub detail_scroll: u16,
    /// Mode détail élargi.
    pub detail_zoom: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(dex: Pokedex, tcg: Option<TcgDatabase>) -> App {
        let mut app = App {
            dex,
            tcg,
            tab: Tab::Pokedex,
            mode: Mode::Normal,
            dex_filters: DexFilters::default(),
            card_filters: CardFilters::default(),
            dex_rows: Vec::new(),
            card_rows: Vec::new(),
            dex_state: ListState::default(),
            card_state: ListState::default(),
            popup: None,
            detail_scroll: 0,
            detail_zoom: false,
            should_quit: false,
        };
        app.apply_filters();
        app.select_first();
        app
    }

    /// Recalcule les listes visibles à partir des filtres courants.
    pub fn apply_filters(&mut self) {
        let df = &self.dex_filters;
        let filter = Filter {
            type_id: df.type_id,
            generation: df.generation,
            legendary_only: df.legendary_only,
            mythical_only: df.mythical_only,
            ..Filter::default()
        };
        self.dex_rows = self
            .dex
            .query_list(&filter, &df.query, df.sort.to_sort_key())
            .into_iter()
            .map(|p| p.id)
            .collect();

        if let Some(tcg) = self.tcg.as_ref() {
            let cf = &self.card_filters;
            let filter = CardFilter {
                query: cf.query.clone(),
                types: cf.type_.clone().into_iter().collect(),
                rarity: cf.rarity.clone(),
                set_id: cf.set_id.clone(),
                ..CardFilter::default()
            };
            self.card_rows = tcg.filter_indices(&filter, cf.sort);
        }

        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        for (state, len) in [
            (&mut self.dex_state, self.dex_rows.len()),
            (&mut self.card_state, self.card_rows.len()),
        ] {
            if len == 0 {
                state.select(None);
            } else {
                let idx = state.selected().unwrap_or(0).min(len - 1);
                state.select(Some(idx));
            }
        }
    }

    fn select_first(&mut self) {
        self.dex_state.select(self.dex_rows.first().map(|_| 0));
        self.card_state.select(self.card_rows.first().map(|_| 0));
    }

    pub fn tcg(&self) -> Option<&TcgDatabase> {
        self.tcg.as_ref()
    }

    pub fn visible_len(&self) -> usize {
        match self.tab {
            Tab::Pokedex => self.dex_rows.len(),
            Tab::Cards => self.card_rows.len(),
        }
    }

    /// Pokémon sélectionné dans l'onglet Pokédex.
    pub fn selected_pokemon(&self) -> Option<&crate::Pokemon> {
        let idx = self.dex_state.selected()?;
        let id = self.dex_rows.get(idx).copied()?;
        self.dex.get(id)
    }

    /// Carte sélectionnée dans l'onglet Cartes TCG.
    pub fn selected_card(&self) -> Option<&crate::tcg::TcgCard> {
        let tcg = self.tcg.as_ref()?;
        let idx = self.card_state.selected()?;
        let row = self.card_rows.get(idx).copied()?;
        tcg.cards().get(row)
    }

    /// Items du popup ouvert, filtrés par sa recherche locale.
    pub fn popup_items(&self) -> Vec<PopupItem> {
        let Some(popup) = self.popup.as_ref() else {
            return Vec::new();
        };
        let items: Vec<PopupItem> = match popup.kind {
            PopupKind::DexType => self
                .dex
                .types()
                .iter()
                .map(|t| PopupItem {
                    key: t.id.to_string(),
                    label: t.fr.clone(),
                })
                .collect(),
            PopupKind::CardType => self
                .tcg()
                .map(|db| {
                    db.card_types
                        .iter()
                        .map(|t| PopupItem {
                            key: t.clone(),
                            label: crate::tcg::energy_fr(t).to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            PopupKind::Rarity => self
                .tcg()
                .map(|db| {
                    db.rarities
                        .iter()
                        .map(|r| PopupItem {
                            key: r.clone(),
                            label: r.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            PopupKind::Set => self
                .tcg()
                .map(|db| {
                    db.sets()
                        .iter()
                        .map(|s| PopupItem {
                            key: s.id.clone(),
                            label: format!("{} — {}", s.series, s.name),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };
        items
            .into_iter()
            .filter(|i| match_label(&popup.query, &i.label) || match_label(&popup.query, &i.key))
            .collect()
    }

    /// Applique la sélection courante du popup au filtre correspondant.
    /// Sélectionner la valeur déjà active la désactive (toggle).
    fn apply_popup_selection(&mut self) {
        let Some(popup) = self.popup.as_ref() else {
            return;
        };
        let Some(item) = self.popup_items().into_iter().nth(popup.selection) else {
            return;
        };
        match popup.kind {
            PopupKind::DexType => {
                let value: Option<u32> = item.key.parse().ok();
                self.dex_filters.type_id = if self.dex_filters.type_id == value {
                    None
                } else {
                    value
                };
            }
            PopupKind::CardType => {
                self.card_filters.type_ =
                    if self.card_filters.type_.as_deref() == Some(item.key.as_str()) {
                        None
                    } else {
                        Some(item.key)
                    };
            }
            PopupKind::Rarity => {
                self.card_filters.rarity =
                    if self.card_filters.rarity.as_deref() == Some(item.key.as_str()) {
                        None
                    } else {
                        Some(item.key)
                    };
            }
            PopupKind::Set => {
                self.card_filters.set_id =
                    if self.card_filters.set_id.as_deref() == Some(item.key.as_str()) {
                        None
                    } else {
                        Some(item.key)
                    };
            }
        }
        self.popup = None;
        self.apply_filters();
        self.select_first();
    }

    // -- Navigation ----------------------------------------------------

    fn move_selection(&mut self, delta: i32) {
        let (state, len) = match self.tab {
            Tab::Pokedex => (&mut self.dex_state, self.dex_rows.len()),
            Tab::Cards => (&mut self.card_state, self.card_rows.len()),
        };
        if len == 0 {
            state.select(None);
            return;
        }
        let current = state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        if next != current as usize {
            self.detail_scroll = 0;
        }
        state.select(Some(next));
    }

    fn reset_filters(&mut self) {
        match self.tab {
            Tab::Pokedex => self.dex_filters = DexFilters::default(),
            Tab::Cards => self.card_filters = CardFilters::default(),
        }
        self.apply_filters();
        self.select_first();
    }

    // -- Événements ----------------------------------------------------

    /// Traite une touche. Point d'entrée unique de la TUI.
    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.should_quit = true;
            return;
        }
        if self.popup.is_some() {
            self.on_popup_key(key);
        } else if self.mode == Mode::Search {
            self.on_search_key(key);
        } else {
            self.on_normal_key(key);
        }
    }

    fn on_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                self.tab = self.tab.toggle();
                self.detail_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-20),
            KeyCode::PageDown => self.move_selection(20),
            KeyCode::Home => self.move_selection(i32::MIN / 2),
            KeyCode::End => self.move_selection(i32::MAX / 2),
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
            }
            KeyCode::Char('t') => self.open_popup(match self.tab {
                Tab::Pokedex => PopupKind::DexType,
                Tab::Cards => PopupKind::CardType,
            }),
            KeyCode::Char('r') if self.tab == Tab::Cards => {
                self.open_popup(PopupKind::Rarity);
            }
            KeyCode::Char('e') if self.tab == Tab::Cards => self.open_popup(PopupKind::Set),
            KeyCode::Char('g') if self.tab == Tab::Pokedex => {
                let df = &mut self.dex_filters;
                df.generation = match df.generation {
                    None => Some(1),
                    Some(9) => None,
                    Some(g) => Some(g + 1),
                };
                self.apply_filters();
                self.select_first();
            }
            KeyCode::Char('l') if self.tab == Tab::Pokedex => {
                self.dex_filters.legendary_only = !self.dex_filters.legendary_only;
                self.apply_filters();
                self.select_first();
            }
            KeyCode::Char('m') if self.tab == Tab::Pokedex => {
                self.dex_filters.mythical_only = !self.dex_filters.mythical_only;
                self.apply_filters();
                self.select_first();
            }
            KeyCode::Char('s') => {
                match self.tab {
                    Tab::Pokedex => self.dex_filters.sort = self.dex_filters.sort.cycle(),
                    Tab::Cards => self.card_filters.sort = self.card_filters.sort.cycle(),
                }
                self.apply_filters();
                self.select_first();
            }
            KeyCode::Char('x') => self.reset_filters(),
            KeyCode::Char('i') => self.detail_zoom = !self.detail_zoom,
            KeyCode::Char(',') => self.detail_scroll = self.detail_scroll.saturating_sub(2),
            KeyCode::Char('.') => self.detail_scroll = self.detail_scroll.saturating_add(2),
            _ => {}
        }
    }

    fn on_search_key(&mut self, key: KeyEvent) {
        let query = match self.tab {
            Tab::Pokedex => &mut self.dex_filters.query,
            Tab::Cards => &mut self.card_filters.query,
        };
        match key.code {
            KeyCode::Enter | KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                query.pop();
                self.apply_filters();
                self.select_first();
            }
            KeyCode::Char(c) => {
                query.push(c);
                self.apply_filters();
                self.select_first();
            }
            _ => {}
        }
    }

    fn open_popup(&mut self, kind: PopupKind) {
        if matches!(
            kind,
            PopupKind::CardType | PopupKind::Rarity | PopupKind::Set
        ) && self.tcg.is_none()
        {
            return;
        }
        self.popup = Some(PopupState::new(kind));
    }

    fn on_popup_key(&mut self, key: KeyEvent) {
        let len = self.popup_items().len();
        let Some(popup) = self.popup.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.popup = None,
            KeyCode::Up => popup.selection = popup.selection.saturating_sub(1),
            KeyCode::Down => {
                if len > 0 {
                    popup.selection = (popup.selection + 1).min(len - 1);
                }
            }
            KeyCode::Enter => self.apply_popup_selection(),
            KeyCode::Backspace => {
                popup.query.pop();
                popup.selection = 0;
            }
            KeyCode::Char(c) => {
                popup.query.push(c);
                popup.selection = 0;
            }
            _ => {}
        }
    }
}

/// Élément affichable dans un popup : clé de filtre + libellé affiché.
#[derive(Debug, Clone)]
pub struct PopupItem {
    pub key: String,
    pub label: String,
}

//! Affichage en ASCII : tableau, fiche détaillée, comparaison.

use crate::db::{Pokedex, StatField};
use crate::Pokemon;

/// Largeur des barres de statistiques.
const BAR_WIDTH: usize = 24;

/// Barre proportionnelle de statistique.
fn bar(value: u32, max: u32) -> String {
    if max == 0 {
        return String::new();
    }
    let filled = ((value as usize).min(max as usize) * BAR_WIDTH / max as usize).min(BAR_WIDTH);
    let mut out = "█".repeat(filled);
    out.push_str(&"·".repeat(BAR_WIDTH - filled));
    out
}

/// Taille en mètres avec virgule française.
fn meters(dm: u32) -> String {
    format!("{:.1} m", f64::from(dm) / 10.0)
        .chars()
        .map(|c| if c == '.' { ',' } else { c })
        .collect()
}

/// Poids en kilogrammes avec virgule française.
fn kilograms(hg: u32) -> String {
    format!("{:.1} kg", f64::from(hg) / 10.0)
        .chars()
        .map(|c| if c == '.' { ',' } else { c })
        .collect()
}

fn types_label(dex: &Pokedex, p: &Pokemon) -> String {
    p.types
        .iter()
        .map(|&t| dex.type_label(t))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Multiplicateur formaté, ex. `×4`, `×0,5`, `×0`.
fn factor_label(m: f32) -> String {
    let rounded = (m * 100.0).round() / 100.0;
    let mut s = if (rounded - rounded.trunc()).abs() < f32::EPSILON {
        format!("×{}", rounded.trunc() as u32)
    } else {
        format!("×{rounded:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    };
    if s.contains('.') {
        s = s.replace('.', ",");
    }
    s
}

/// Ligne de tableau : id, nom, types, génération, total.
pub fn table(dex: &Pokedex, rows: &[&Pokemon]) -> String {
    let name_w = rows
        .iter()
        .map(|p| p.name_fr.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 18);
    let types_w = rows
        .iter()
        .map(|p| types_label(dex, p).chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 20);
    let mut out = format!(
        "{:<5} {:<width$} {:<tw$} {:>4} {:>6}\n",
        "N°",
        "Nom",
        "Types",
        "Gen",
        "Total",
        width = name_w,
        tw = types_w
    );
    out.push_str(&"-".repeat(name_w + types_w + 19));
    out.push('\n');
    for p in rows {
        out.push_str(&format!(
            "{:<5} {:<width$} {:<tw$} {:>4} {:>6}\n",
            format!("#{}", p.id),
            p.name_fr,
            types_label(dex, p),
            p.generation,
            p.stats.total(),
            width = name_w,
            tw = types_w
        ));
    }
    out
}

/// Liste paginée avec entête récapitulatif.
pub fn paginated(dex: &Pokedex, rows: &[&Pokemon], page: usize, per_page: usize) -> String {
    let total_pages = rows.len().div_ceil(per_page).max(1);
    let page = page.clamp(1, total_pages);
    let start = (page - 1) * per_page;
    let slice: Vec<&Pokemon> = rows.iter().skip(start).take(per_page).copied().collect();
    let mut out = format!(
        "{} résultats — page {}/{}\n\n",
        rows.len(),
        page,
        total_pages
    );
    out.push_str(&table(dex, &slice));
    out.push_str("\n(suite : `liste <numéro de page>`)");
    out
}

/// Fiche détaillée d'un Pokémon.
pub fn card(dex: &Pokedex, p: &Pokemon) -> String {
    let mut out = String::new();
    let title = format!("#{:04} {} ({})", p.id, p.name_fr, p.name_en);
    let width = title.chars().count().max(46);
    out.push_str(&"─".repeat(width + 2));
    out.push('\n');
    out.push_str(&format!(" {} \n", title));
    if !p.genus_fr.is_empty() {
        out.push_str(&format!(" {}\n", p.genus_fr));
    }
    out.push_str(&"─".repeat(width + 2));
    out.push('\n');

    let mut tags: Vec<&str> = Vec::new();
    if p.is_legendary {
        tags.push("Légendaire");
    }
    if p.is_mythical {
        tags.push("Mythique");
    }
    let tag_line = if tags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", tags.join(" · "))
    };
    out.push_str(&format!(
        " Génération {} · Types : {}{}\n",
        p.generation,
        types_label(dex, p),
        tag_line
    ));
    out.push_str(&format!(
        " Taille : {} · Poids : {}\n\n",
        meters(p.height_dm),
        kilograms(p.weight_hg)
    ));

    out.push_str(" Stats de base\n");
    let stat_lines: [(&str, u32); 6] = [
        ("PV", u32::from(p.stats.hp)),
        ("Attaque", u32::from(p.stats.attack)),
        ("Défense", u32::from(p.stats.defense)),
        ("Atq. spé", u32::from(p.stats.special_attack)),
        ("Déf. spé", u32::from(p.stats.special_defense)),
        ("Vitesse", u32::from(p.stats.speed)),
    ];
    for (label, v) in stat_lines {
        out.push_str(&format!(
            "  {:<10} {:<26} {:>3}/255\n",
            label,
            bar(v, 255),
            v
        ));
    }
    out.push_str(&format!(
        "  {:<10} {:<26} {:>3}\n",
        "Total",
        bar(p.stats.total(), Pokedex::stat_max(StatField::Total)),
        p.stats.total()
    ));

    out.push_str("\n Efficacité des types contre lui\n");
    let groups = [
        (4.0, "Ultra-faiblesse"),
        (2.0, "Faiblesses"),
        (0.5, "Résistances"),
        (0.25, "Ultra-résistances"),
    ];
    for (target, label) in groups {
        let names: Vec<String> = dex
            .multipliers(p)
            .into_iter()
            .filter(|(_, m)| (*m - target).abs() < f32::EPSILON)
            .map(|(t, _)| t.fr.clone())
            .collect();
        if !names.is_empty() {
            out.push_str(&format!("  {:<18} {}\n", label, names.join(", ")));
        }
    }
    let immunities: Vec<String> = dex
        .multipliers(p)
        .into_iter()
        .filter(|(_, m)| *m == 0.0)
        .map(|(t, _)| t.fr.clone())
        .collect();
    if !immunities.is_empty() {
        out.push_str(&format!(
            "  {:<18} {}\n",
            "Immunités",
            immunities.join(", ")
        ));
    }
    out
}

/// Comparaison côte à côte de deux Pokémon.
pub fn compare(dex: &Pokedex, a: &Pokemon, b: &Pokemon) -> String {
    let name_w = a
        .name_fr
        .chars()
        .count()
        .max(b.name_fr.chars().count())
        .max(6);
    let mut out = String::from("Comparaison\n");
    out.push_str(&"-".repeat(name_w * 2 + 16));
    out.push('\n');
    // Colonne a alignée à droite, marqueur centré, colonne b alignée à gauche :
    // « < » désigne le gagnant de gauche, « > » celui de droite.
    let row = |label: &str, a: &str, b: &str, marker: &str| {
        format!(
            " {:<10} {:>width$} {:^3} {:<width$}\n",
            label,
            a,
            marker,
            b,
            width = name_w
        )
    };
    out.push_str(&row("", &format!("#{}", a.id), &format!("#{}", b.id), ""));
    out.push_str(&row("Nom", &a.name_fr, &b.name_fr, ""));
    out.push_str(&row(
        "Types",
        &types_label(dex, a),
        &types_label(dex, b),
        "",
    ));
    out.push_str(&row(
        "Génération",
        &a.generation.to_string(),
        &b.generation.to_string(),
        "",
    ));
    let stat_rows: [(&str, u32, u32); 6] = [
        ("PV", u32::from(a.stats.hp), u32::from(b.stats.hp)),
        (
            "Attaque",
            u32::from(a.stats.attack),
            u32::from(b.stats.attack),
        ),
        (
            "Défense",
            u32::from(a.stats.defense),
            u32::from(b.stats.defense),
        ),
        (
            "Atq. spé",
            u32::from(a.stats.special_attack),
            u32::from(b.stats.special_attack),
        ),
        (
            "Déf. spé",
            u32::from(a.stats.special_defense),
            u32::from(b.stats.special_defense),
        ),
        (
            "Vitesse",
            u32::from(a.stats.speed),
            u32::from(b.stats.speed),
        ),
    ];
    for (label, va, vb) in stat_rows {
        let marker = if va > vb {
            "<"
        } else if vb > va {
            ">"
        } else {
            "="
        };
        out.push_str(&row(label, &va.to_string(), &vb.to_string(), marker));
    }
    let (ta, tb) = (a.stats.total(), b.stats.total());
    let marker = if ta > tb {
        "<"
    } else if tb > ta {
        ">"
    } else {
        "="
    };
    out.push_str(&row("Total", &ta.to_string(), &tb.to_string(), marker));
    out
}

/// Fiche d'un type : forces et faiblesses offensives et défensives.
pub fn type_card(dex: &Pokedex, type_id: u32) -> String {
    let label = dex.type_label(type_id);
    let mut out = format!("Type « {label} »\n\n Offensivement\n");
    let mut strong = Vec::new();
    let mut weak = Vec::new();
    let mut zero = Vec::new();
    for target in dex.types() {
        let f = dex.efficacy(type_id, target.id);
        if f > 100 {
            strong.push(format!("{} {}", target.fr, factor_label(f as f32 / 100.0)));
        } else if f == 0 {
            zero.push(target.fr.clone());
        } else if f < 100 {
            weak.push(format!("{} {}", target.fr, factor_label(f as f32 / 100.0)));
        }
    }
    out.push_str(&format!("  Efficace contre   {}\n", strong.join(", ")));
    if !weak.is_empty() {
        out.push_str(&format!("  Peu efficace      {}\n", weak.join(", ")));
    }
    if !zero.is_empty() {
        out.push_str(&format!("  Sans effet        {}\n", zero.join(", ")));
    }
    out.push_str("\n En défense\n");
    let mut vulnerable = Vec::new();
    let mut resistant = Vec::new();
    let mut immune = Vec::new();
    for attacker in dex.types() {
        let f = dex.efficacy(attacker.id, type_id);
        if f > 100 {
            vulnerable.push(format!(
                "{} {}",
                attacker.fr,
                factor_label(f as f32 / 100.0)
            ));
        } else if f == 0 {
            immune.push(attacker.fr.clone());
        } else if f < 100 {
            resistant.push(format!(
                "{} {}",
                attacker.fr,
                factor_label(f as f32 / 100.0)
            ));
        }
    }
    out.push_str(&format!("  Faible contre     {}\n", vulnerable.join(", ")));
    if !resistant.is_empty() {
        out.push_str(&format!("  Résiste à         {}\n", resistant.join(", ")));
    }
    if !immune.is_empty() {
        out.push_str(&format!("  Immunisé à        {}\n", immune.join(", ")));
    }
    out
}

/// Multiplicateur de dégâts d'un type attaquant sur un type défenseur
/// (pour l'affichage dans les tests et le debug).
pub fn factor(factor: f32) -> String {
    factor_label(factor)
}

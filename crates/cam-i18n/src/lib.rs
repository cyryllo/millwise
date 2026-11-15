//! Tłumaczenia interfejsu. Tekstem źródłowym (kluczem) jest napis polski;
//! słownik angielski jest w `en.json` (polski → angielski). Brak wpisu =
//! napis polski. Miejsca na wartości: `{}` oraz `{:.N}` (liczba miejsc po
//! przecinku), `{:>N}` / `{:<N}` (szerokość); `{{` i `}}` to nawiasy.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Pl,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Pl, Lang::En];

    /// Nazwa języka w nim samym.
    pub fn name(&self) -> &'static str {
        match self {
            Lang::Pl => "Polski",
            Lang::En => "English",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::Pl => "pl",
            Lang::En => "en",
        }
    }

    pub fn from_code(c: &str) -> Option<Lang> {
        match c {
            "pl" => Some(Lang::Pl),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// Język systemu (LANG, LC_ALL, LC_MESSAGES): polski dla „pl…”, inaczej angielski.
    pub fn from_system() -> Lang {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(v) = std::env::var(var) {
                if !v.is_empty() {
                    return if v.to_lowercase().starts_with("pl") { Lang::Pl } else { Lang::En };
                }
            }
        }
        Lang::En
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(l: Lang) {
    LANG.store(if l == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 { Lang::En } else { Lang::Pl }
}

/// Słownik angielski (polski → angielski).
pub const EN_JSON: &str = include_str!("../en.json");

fn en() -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    EN.get_or_init(|| serde_json::from_str(EN_JSON).expect("cam-i18n: niepoprawny en.json"))
}

/// Tłumaczenie napisu (dla napisów stałych).
pub fn tr(s: &'static str) -> &'static str {
    match lang() {
        Lang::Pl => s,
        Lang::En => en().get(s).map(|t| t.as_str()).filter(|t| !t.is_empty()).unwrap_or(s),
    }
}

/// Tłumaczenie napisu znanego dopiero w czasie działania (np. nazwy materiału).
pub fn tr_dyn(s: &str) -> String {
    match lang() {
        Lang::Pl => s.to_string(),
        Lang::En => en().get(s).filter(|t| !t.is_empty()).cloned().unwrap_or_else(|| s.to_string()),
    }
}

/// Wstawia wartości w miejsca `{}` / `{:spec}` szablonu.
pub fn fmt(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut it = template.chars().peekable();
    let mut next = 0;
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut spec = String::new();
                for d in it.by_ref() {
                    if d == '}' {
                        break;
                    }
                    spec.push(d);
                }
                let Some(arg) = args.get(next) else { continue };
                next += 1;
                out.push_str(&format_arg(*arg, spec.strip_prefix(':').unwrap_or("")));
            }
            c => out.push(c),
        }
    }
    out
}

/// Formatowanie jednej wartości według `spec`: [<>^][szerokość][.precyzja].
fn format_arg(arg: &dyn Display, spec: &str) -> String {
    // `{:#}` — postać rozszerzona (np. pełny łańcuch przyczyn błędu)
    if spec == "#" {
        return format!("{arg:#}");
    }
    let (align, rest) = match spec.chars().next() {
        Some(c @ ('<' | '>' | '^')) => (Some(c), &spec[1..]),
        _ => (None, spec),
    };
    let (width, prec) = match rest.split_once('.') {
        Some((w, p)) => (w.parse::<usize>().ok(), p.parse::<usize>().ok()),
        None => (rest.parse::<usize>().ok(), None),
    };
    let s = match prec {
        Some(p) => format!("{arg:.p$}"),
        None => format!("{arg}"),
    };
    match width {
        Some(w) => match align {
            Some('<') => format!("{s:<w$}"),
            Some('^') => format!("{s:^w$}"),
            Some('>') => format!("{s:>w$}"),
            // domyślnie jak w Rust: liczby do prawej, tekst do lewej
            _ => {
                if s.parse::<f64>().is_ok() {
                    format!("{s:>w$}")
                } else {
                    format!("{s:<w$}")
                }
            }
        },
        None => s,
    }
}

/// `tr!("napis")` → &'static str; `tr!("szablon {} {:.1}", a, b)` → String.
#[macro_export]
macro_rules! tr {
    ($s:literal) => {
        $crate::tr($s)
    };
    ($s:literal, $($a:expr),+ $(,)?) => {
        $crate::fmt($crate::tr($s), &[$(&$a as &dyn ::std::fmt::Display),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_and_specs() {
        assert_eq!(fmt("a {} b {:.2} c {{x}}", &[&1, &2.5_f64]), "a 1 b 2.50 c {x}");
        assert_eq!(fmt("[{:>5}] [{:<4}] [{:>7.1}]", &[&"ab", &"x", &3.14159_f64]), "[   ab] [x   ] [    3.1]");
        assert_eq!(fmt("{} {}", &[&"tylko"]), "tylko ");
    }

    #[test]
    fn dictionary_parses_and_languages() {
        assert!(!en().is_empty());
        set_lang(Lang::Pl);
        assert_eq!(tr("Plik"), "Plik");
        assert_eq!(Lang::from_code("en"), Some(Lang::En));
    }
}

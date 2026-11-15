//! Test kompletności tłumaczeń: każdy napis w `tr!("…")` w kodzie programu
//! musi mieć wpis w słowniku angielskim, z tymi samymi miejscami na wartości.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// Napis Rust zaczynający się w `s` (po cudzysłowie) — z obsługą `\\`, `\"`, `\n` i łamania linii.
    fn parse_literal(s: &str) -> Option<String> {
        let mut out = String::new();
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '"' => return Some(out),
                '\\' => match it.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '\'' => out.push('\''),
                    '\n' => {
                        while it.peek().is_some_and(|c| c.is_whitespace()) {
                            it.next();
                        }
                    }
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                },
                c => out.push(c),
            }
        }
        None
    }

    fn placeholders(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            if c == '{' {
                if it.peek() == Some(&'{') {
                    it.next();
                    continue;
                }
                let mut p = String::from("{");
                for d in it.by_ref() {
                    p.push(d);
                    if d == '}' {
                        break;
                    }
                }
                out.push(p);
            } else if c == '}' && it.peek() == Some(&'}') {
                it.next();
            }
        }
        out
    }

    #[test]
    fn every_translated_text_has_an_english_entry() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        rust_files(&root.join("app/src"), &mut files);
        for c in ["cam-io", "cam-ops", "cam-post", "cam-sim", "cam-geom"] {
            rust_files(&root.join("crates").join(c).join("src"), &mut files);
        }
        let en: HashMap<String, String> = serde_json::from_str(cam_i18n::EN_JSON).unwrap();
        let mut missing = Vec::new();
        let mut checked = 0;
        for f in files {
            if f.ends_with("i18n_check.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            let mut rest = text.as_str();
            while let Some(i) = rest.find("tr!(") {
                // całe słowo `tr!`, nie koniec np. `include_str!`
                let word = i == 0 || !rest[..i].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_');
                rest = &rest[i + 4..];
                if !word {
                    continue;
                }
                let trimmed = rest.trim_start();
                let Some(lit) = trimmed.strip_prefix('"') else { continue };
                let Some(key) = parse_literal(lit) else { continue };
                checked += 1;
                match en.get(&key) {
                    None => missing.push(format!("{}: brak tłumaczenia: {key:?}", f.display())),
                    Some(v) if placeholders(v) != placeholders(&key) => {
                        missing.push(format!("{}: inne miejsca na wartości: {key:?} → {v:?}", f.display()))
                    }
                    Some(v) if v.trim().is_empty() => missing.push(format!("{}: puste tłumaczenie: {key:?}", f.display())),
                    _ => {}
                }
            }
        }
        assert!(checked > 400, "znaleziono tylko {checked} napisów — test nie widzi kodu?");
        assert!(missing.is_empty(), "{} problemów:\n{}", missing.len(), missing.join("\n"));
    }
}

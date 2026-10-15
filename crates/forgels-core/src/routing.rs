use std::path::Path;

/// Language supported by the server.
///
/// Mirrors `LANGUAGE_ID` of each `forgels-lang-*` crate without depending
/// on them. Shared crates must never import language crates, so this table
/// is the single routing entry point: one new language means one new arm
/// in [`route_by_extension`] plus one in [`route_by_language_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Solidity,
    Noir,
}

impl Language {
    /// Canonical id sent by editors, for example `solidity`.
    pub fn id(self) -> &'static str {
        match self {
            Language::Solidity => "solidity",
            Language::Noir => "noir",
        }
    }

    /// File extension without dot, for example `sol`.
    pub fn extension(self) -> &'static str {
        match self {
            Language::Solidity => "sol",
            Language::Noir => "nr",
        }
    }
}

/// Route by file extension. Returns `None` for unsupported files.
pub fn route_by_extension(path: &Path) -> Option<Language> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "sol" => Some(Language::Solidity),
        "nr" => Some(Language::Noir),
        _ => None,
    }
}

/// Route by LSP language id. Returns `None` for unsupported ids.
pub fn route_by_language_id(id: &str) -> Option<Language> {
    match id {
        "solidity" => Some(Language::Solidity),
        "noir" => Some(Language::Noir),
        _ => None,
    }
}

/// Central route for an opened file.
///
/// Trusts an explicit language id first, falls back to the path extension.
/// Returns `None` when neither is known, so callers can ignore the file.
pub fn route_file(path: &Path, language_id: Option<&str>) -> Option<Language> {
    if let Some(id) = language_id
        && let Some(lang) = route_by_language_id(id)
    {
        return Some(lang);
    }
    route_by_extension(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sol_routes_to_solidity() {
        assert_eq!(
            route_file(&PathBuf::from("contracts/Token.sol"), None),
            Some(Language::Solidity)
        );
    }

    #[test]
    fn nr_routes_to_noir() {
        assert_eq!(
            route_file(&PathBuf::from("circuits/main.nr"), None),
            Some(Language::Noir)
        );
    }

    #[test]
    fn language_id_routes_without_path_hint() {
        assert_eq!(
            route_file(&PathBuf::from("unknown.txt"), Some("solidity")),
            Some(Language::Solidity)
        );
        assert_eq!(
            route_file(&PathBuf::from("unknown.txt"), Some("noir")),
            Some(Language::Noir)
        );
    }

    #[test]
    fn language_id_wins_over_extension() {
        assert_eq!(
            route_file(&PathBuf::from("odd.sol"), Some("noir")),
            Some(Language::Noir)
        );
    }

    #[test]
    fn unknown_extension_falls_back_to_id_then_none() {
        assert_eq!(route_file(&PathBuf::from("notes.md"), None), None);
        assert_eq!(
            route_file(&PathBuf::from("notes.md"), Some("markdown")),
            None
        );
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert_eq!(
            route_file(&PathBuf::from("contracts/Token.SOL"), None),
            Some(Language::Solidity)
        );
    }

    #[test]
    fn language_table_matches_crates() {
        assert_eq!(Language::Solidity.id(), "solidity");
        assert_eq!(Language::Solidity.extension(), "sol");
        assert_eq!(Language::Noir.id(), "noir");
        assert_eq!(Language::Noir.extension(), "nr");
    }
}

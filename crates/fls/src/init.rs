use std::path::Path;

use forgels_lint::RuleId;

use crate::cli::InitArgs;

// Scaffold ./fls.json with explicit defaults: all lint rules at err,
// lint includes `**/*.sol`, empty excludes, plus an explicit format
// group (line length 120, tab width 4, space style).
pub fn scaffold(cwd: &Path, args: &InitArgs) -> i32 {
    let path = cwd.join("fls.json");
    if path.exists() && !args.force {
        eprintln!("fls: fls.json already exists, use --force to overwrite");
        return 2;
    }
    let mut rules = serde_json::Map::new();
    for rule in RuleId::all() {
        rules.insert(
            rule.as_str().to_string(),
            serde_json::Value::String("err".to_string()),
        );
    }
    let root = serde_json::json!({
        "lint": {
            "includes": ["**/*.sol"],
            "excludes": [],
            "rules": rules,
        },
        "format": {
            "includes": [],
            "excludes": [],
            "line_length": 120,
            "tab_width": 4,
            "style": "space",
            "rules": {},
        },
    });
    let Ok(pretty) = serde_json::to_string_pretty(&root) else {
        eprintln!("fls: cannot encode fls.json");
        return 2;
    };
    if let Err(err) = std::fs::write(&path, pretty) {
        eprintln!("fls: cannot write {}: {err}", path.display());
        return 2;
    }
    0
}

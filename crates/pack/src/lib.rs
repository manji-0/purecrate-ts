use purecrate_emit_ts::{emit, File as TsFile, Package};
use purecrate_ir::Crate;

pub fn assemble(krate: &Crate) -> Package {
    let mut pkg = emit(krate);
    pkg.files.insert(
        0,
        TsFile {
            stem: "package.json".to_string(),
            source: package_json(krate.name.as_str()),
        },
    );
    pkg.files.insert(
        1,
        TsFile {
            stem: "tsconfig.json".to_string(),
            source: tsconfig(),
        },
    );
    pkg
}

pub fn disk_path(stem: &str) -> String {
    if stem.ends_with(".json") {
        stem.to_string()
    } else {
        format!("src/{stem}.ts")
    }
}

fn package_json(name: &str) -> String {
    let kebab = purecrate_ir::to_kebab(name);
    format!(
        "{{\n  \"name\": \"{kebab}\",\n  \"type\": \"module\",\n  \"exports\": \"./src/index.ts\"\n}}\n"
    )
}

fn tsconfig() -> String {
    "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    \"target\": \"ES2022\",\n    \"module\": \"ES2022\",\n    \"moduleResolution\": \"bundler\",\n    \"noEmit\": true\n  },\n  \"include\": [\"src\"]\n}\n"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use purecrate_ir::counter_example;

    #[test]
    fn counter_package_has_manifest_and_sources() {
        let pkg = assemble(&counter_example());
        let stems: Vec<&str> = pkg.files.iter().map(|f| f.stem.as_str()).collect();
        assert!(stems.contains(&"package.json"));
        assert!(stems.contains(&"event"));
        assert_eq!(disk_path("event"), "src/event.ts");
        assert_eq!(disk_path("package.json"), "package.json");
    }
}

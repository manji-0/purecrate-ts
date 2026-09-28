use purecrate_emit_ts::{emit, emit_wire, File as TsFile, Package, WireSchema};
use purecrate_ir::Crate;

pub fn assemble(krate: &Crate) -> Package {
    assemble_with(krate, None)
}

pub fn assemble_with(krate: &Crate, schema: Option<WireSchema>) -> Package {
    let mut pkg = emit(krate);
    if let Some(schema) = schema {
        pkg.files.push(TsFile {
            stem: "purecrate-wire".to_string(),
            source: emit_wire(krate, schema),
        });
    }
    pkg.files.insert(
        0,
        TsFile {
            stem: "package.json".to_string(),
            source: package_json(krate.name.as_str(), schema),
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

fn package_json(name: &str, schema: Option<WireSchema>) -> String {
    let kebab = purecrate_ir::to_kebab(name);
    let mut deps = vec!["    \"purecrate\": \"0.1.0\"".to_string()];
    if let Some(schema) = schema {
        deps.push(format!("    \"{}\": \"{}\"", schema.runtime_dep(), schema.version()));
        deps.push(format!("    \"{}\": \"0.1.0\"", schema.package()));
    }
    format!(
        "{{\n  \"name\": \"{kebab}\",\n  \"type\": \"module\",\n  \"exports\": \"./src/index.ts\",\n  \"dependencies\": {{\n{}\n  }}\n}}\n",
        deps.join(",\n")
    )
}

fn tsconfig() -> String {
    "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    \"target\": \"ES2022\",\n    \"module\": \"ES2022\",\n    \"moduleResolution\": \"bundler\",\n    \"allowImportingTsExtensions\": true,\n    \"skipLibCheck\": true,\n    \"noEmit\": true\n  },\n  \"include\": [\"src\"]\n}\n"
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

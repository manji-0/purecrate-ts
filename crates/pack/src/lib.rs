use purecrate_emit_ts::{emit, emit_wire, File as TsFile, Package, WireSchema};
use purecrate_ir::Crate;

/// The version a package gets when the crate's manifest names none.
pub const DEFAULT_VERSION: &str = "0.1.0";

/// The runtime (`purecrate`) and adapter (`purecrate-*`) versions a generated
/// package accepts.
const RUNTIME_RANGE: &str = "^0.1.0";

/// The TypeScript majors the package builds and type-checks with.
const TYPESCRIPT_RANGE: &str = "^6.0.0 || ^7.0.0";

/// Condition that resolves a package to its TypeScript sources instead of
/// `dist`, for working in this repository without building first.
pub const SOURCE_CONDITION: &str = "purecrate-source";

pub fn assemble(krate: &Crate) -> Package {
    assemble_with(krate, None)
}

pub fn assemble_with(krate: &Crate, schema: Option<WireSchema>) -> Package {
    assemble_versioned(krate, schema, DEFAULT_VERSION)
}

/// The TS sources plus a manifest that makes them an npm package: `build`
/// (run by `prepack`) compiles them to `dist`, which `exports` points at, and
/// the runtime is a peer dependency so every package shares its brands.
pub fn assemble_versioned(krate: &Crate, schema: Option<WireSchema>, version: &str) -> Package {
    let mut pkg = emit(krate);
    if let Some(schema) = schema {
        pkg.files.push(TsFile {
            stem: "purecrate-wire".to_string(),
            source: emit_wire(krate, schema),
        });
    }
    let manifests = [
        ("package.json", package_json(krate.name.as_str(), version, schema)),
        ("tsconfig.json", tsconfig()),
        ("tsconfig.build.json", tsconfig_build()),
    ];
    for (i, (stem, source)) in manifests.into_iter().enumerate() {
        pkg.files.insert(i, TsFile { stem: stem.to_string(), source });
    }
    pkg
}

pub fn disk_path(stem: &str) -> String {
    if stem.ends_with(".json") {
        stem.to_string()
    } else {
        format!("src/{stem}.ts")
    }
}

fn entry(stem: &str) -> String {
    format!(
        "{{\n      \"{SOURCE_CONDITION}\": \"./src/{stem}.ts\",\n      \"types\": \"./dist/{stem}.d.ts\",\n      \"default\": \"./dist/{stem}.js\"\n    }}"
    )
}

fn package_json(name: &str, version: &str, schema: Option<WireSchema>) -> String {
    let kebab = purecrate_ir::to_kebab(name);
    let mut exports = vec![format!("    \".\": {}", entry("index"))];
    let mut peers = vec![format!("    \"purecrate\": \"{RUNTIME_RANGE}\"")];
    if let Some(schema) = schema {
        exports.push(format!("    \"./wire\": {}", entry("purecrate-wire")));
        peers.push(format!("    \"{}\": \"{RUNTIME_RANGE}\"", schema.package()));
        peers.push(format!("    \"{}\": \"^{}\"", schema.runtime_dep(), schema.version()));
    }
    format!(
        "{{\n  \"name\": \"{kebab}\",\n  \"version\": \"{version}\",\n  \"type\": \"module\",\n  \"exports\": {{\n{exports}\n  }},\n  \"files\": [\"dist\", \"src\"],\n  \"scripts\": {{\n    \"build\": \"tsc -p tsconfig.build.json\",\n    \"prepack\": \"npm run build\"\n  }},\n  \"peerDependencies\": {{\n{peers}\n  }},\n  \"devDependencies\": {{\n    \"typescript\": \"{TYPESCRIPT_RANGE}\"\n  }}\n}}\n",
        exports = exports.join(",\n"),
        peers = peers.join(",\n"),
    )
}

fn tsconfig() -> String {
    "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    \"target\": \"ES2022\",\n    \"module\": \"ES2022\",\n    \"moduleResolution\": \"bundler\",\n    \"allowImportingTsExtensions\": true,\n    \"skipLibCheck\": true,\n    \"noEmit\": true\n  },\n  \"include\": [\"src\"]\n}\n"
        .to_string()
}

/// `dist`: JavaScript with `./x.ts` imports rewritten to `./x.js`, and
/// declarations. Consumers read it without a TypeScript loader.
fn tsconfig_build() -> String {
    "{\n  \"extends\": \"./tsconfig.json\",\n  \"compilerOptions\": {\n    \"noEmit\": false,\n    \"declaration\": true,\n    \"rewriteRelativeImportExtensions\": true,\n    \"rootDir\": \"src\",\n    \"outDir\": \"dist\"\n  }\n}\n"
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
        assert!(stems.contains(&"tsconfig.build.json"));
        assert!(stems.contains(&"event"));
        assert_eq!(disk_path("event"), "src/event.ts");
        assert_eq!(disk_path("package.json"), "package.json");
    }
}

use purecrate_emit_ts::{emit, emit_wire, File as TsFile, Package, WireSchema, HEADER};
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

/// Whether the generated `package.json` lets `npm publish` through. The code
/// purecrate-ts serves is usually private, so a package is `Private` unless
/// asked otherwise: `npm pack` and installing the tarball work, `npm publish`
/// refuses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    #[default]
    Private,
    Publishable,
}

/// Where the runtime comes from. `Peer`: the package depends on `purecrate`,
/// which every generated package in a project shares, so values cross
/// between them. `Bundled`: the runtime's source is copied into the package
/// as `src/purecrate-runtime.ts`, so the sources stand alone, for projects
/// that vendor generated code rather than install it. Brands are then the
/// package's own: its values do not type-check against another package's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Runtime {
    #[default]
    Peer,
    Bundled,
}

/// The file a bundled runtime is written to (`src/<stem>.ts`). `check`
/// reserves the name.
pub const BUNDLED_RUNTIME_STEM: &str = "purecrate-runtime";

const RUNTIME_SOURCE: &str = include_str!("../../../packages/boundary/src/index.ts");

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
    assemble_with_access(krate, schema, version, Access::default())
}

pub fn assemble_with_access(krate: &Crate, schema: Option<WireSchema>, version: &str, access: Access) -> Package {
    assemble_packaged(krate, schema, version, access, Runtime::Peer)
}

/// `assemble_with_access`, choosing where the runtime comes from. A bundled
/// runtime does not go with `schema`: the adapters import `purecrate`.
pub fn assemble_packaged(
    krate: &Crate,
    schema: Option<WireSchema>,
    version: &str,
    access: Access,
    runtime: Runtime,
) -> Package {
    assert!(
        runtime == Runtime::Peer || schema.is_none(),
        "a bundled runtime does not go with a schema adapter"
    );
    let mut pkg = emit(krate);
    if let Some(schema) = schema {
        pkg.files.push(TsFile {
            stem: "purecrate-wire".to_string(),
            source: emit_wire(krate, schema),
        });
    }
    if runtime == Runtime::Bundled {
        let local = format!("from \"./{BUNDLED_RUNTIME_STEM}.ts\";");
        for file in &mut pkg.files {
            file.source = file.source.replace("from \"purecrate\";", &local);
        }
        pkg.files.push(TsFile {
            stem: BUNDLED_RUNTIME_STEM.to_string(),
            source: format!(
                "{HEADER}// The purecrate runtime (MIT, https://github.com/manji-0/purecrate-ts,\n\
                 // packages/boundary), copied in by `build --bundle-runtime`.\n\n{RUNTIME_SOURCE}"
            ),
        });
    }
    let manifests = [
        ("package.json", package_json(krate.name.as_str(), version, schema, access, runtime)),
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

fn package_json(name: &str, version: &str, schema: Option<WireSchema>, access: Access, runtime: Runtime) -> String {
    let kebab = purecrate_ir::to_kebab(name);
    let mut exports = vec![format!("    \".\": {}", entry("index"))];
    let mut peers = Vec::new();
    if runtime == Runtime::Peer {
        peers.push(format!("    \"purecrate\": \"{RUNTIME_RANGE}\""));
    }
    if let Some(schema) = schema {
        exports.push(format!("    \"./wire\": {}", entry("purecrate-wire")));
        peers.push(format!("    \"{}\": \"{RUNTIME_RANGE}\"", schema.package()));
        peers.push(format!("    \"{}\": \"^{}\"", schema.runtime_dep(), schema.version()));
    }
    let private = match access {
        Access::Private => "  \"private\": true,\n",
        Access::Publishable => "",
    };
    let peers = if peers.is_empty() {
        String::new()
    } else {
        format!("  \"peerDependencies\": {{\n{}\n  }},\n", peers.join(",\n"))
    };
    format!(
        "{{\n  \"name\": \"{kebab}\",\n  \"version\": \"{version}\",\n{private}  \"type\": \"module\",\n  \"exports\": {{\n{exports}\n  }},\n  \"files\": [\"dist\", \"src\"],\n  \"scripts\": {{\n    \"build\": \"tsc -p tsconfig.build.json\",\n    \"prepack\": \"npm run build\"\n  }},\n{peers}  \"devDependencies\": {{\n    \"typescript\": \"{TYPESCRIPT_RANGE}\"\n  }}\n}}\n",
        exports = exports.join(",\n"),
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

    fn manifest(access: Access) -> String {
        let pkg = assemble_with_access(&counter_example(), None, "1.2.3", access);
        pkg.files.into_iter().find(|f| f.stem == "package.json").unwrap().source
    }

    #[test]
    fn packages_are_private_unless_publishable() {
        assert!(manifest(Access::default()).contains("  \"private\": true,\n"));
        assert!(!manifest(Access::Publishable).contains("private"));
        assert_eq!(manifest(Access::default()), manifest(Access::Private));
    }

    #[test]
    fn a_bundled_runtime_replaces_the_purecrate_dependency() {
        let pkg = assemble_packaged(&counter_example(), None, "1.2.3", Access::default(), Runtime::Bundled);
        let runtime = pkg.files.iter().find(|f| f.stem == BUNDLED_RUNTIME_STEM).expect("runtime file");
        assert!(runtime.source.starts_with(HEADER) && runtime.source.contains("export const Int = {"));
        for f in &pkg.files {
            assert!(!f.source.contains("from \"purecrate\""), "{} still imports purecrate", f.stem);
        }
        let manifest = &pkg.files.iter().find(|f| f.stem == "package.json").unwrap().source;
        assert!(!manifest.contains("peerDependencies"), "{manifest}");
        let peer = manifest_of(Runtime::Peer);
        assert!(peer.contains("\"purecrate\": \"^0.1.0\""), "{peer}");
    }

    fn manifest_of(runtime: Runtime) -> String {
        let pkg = assemble_packaged(&counter_example(), None, "1.2.3", Access::default(), runtime);
        pkg.files.into_iter().find(|f| f.stem == "package.json").unwrap().source
    }

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

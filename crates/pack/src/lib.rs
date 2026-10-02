use purecrate_emit_ts::{emit, emit_wire, wrap_source, File as TsFile, Package, WireSchema, HEADER};
use purecrate_ir::Crate;

mod trim;

/// The version a package gets when the crate's manifest names none.
pub const DEFAULT_VERSION: &str = "0.1.0";

/// The license a package gets when the crate's manifest names none.
/// Matches the generator and the copied runtime.
pub const DEFAULT_LICENSE: &str = "MIT";

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

/// The file the runtime is copied to (`src/<stem>.ts`). `check` reserves it,
/// and the adapters' stems (`purecrate-zod`, ...).
pub const RUNTIME_STEM: &str = "purecrate-runtime";

/// The runtime and schema adapters, at the revision of this binary. Every
/// package carries its own copy: there is nothing to install beside it but
/// the schema library, and the copy cannot fall out of step with the code
/// generated against it. The runtime's brands are keyed by string, so values
/// still pass between packages that each carry a copy. The copy keeps only
/// the parts the package's code uses (`trim`).
const RUNTIME_SOURCE: &str = include_str!("../../../packages/boundary/src/index.ts");

fn adapter_source(schema: WireSchema) -> &'static str {
    match schema {
        WireSchema::Zod => include_str!("../../../packages/boundary-zod/src/index.ts"),
        WireSchema::Valibot => include_str!("../../../packages/boundary-valibot/src/index.ts"),
        WireSchema::Arktype => include_str!("../../../packages/boundary-arktype/src/index.ts"),
    }
}

pub fn assemble(krate: &Crate) -> Package {
    assemble_with(krate, None)
}

pub fn assemble_with(krate: &Crate, schema: Option<WireSchema>) -> Package {
    assemble_versioned(krate, schema, DEFAULT_VERSION)
}

/// The TS sources plus a manifest that makes them an npm package: `build`
/// (run by `prepack`) compiles them to `dist`, which `exports` points at.
pub fn assemble_versioned(krate: &Crate, schema: Option<WireSchema>, version: &str) -> Package {
    assemble_with_access(krate, schema, version, Access::default())
}

pub fn assemble_with_access(krate: &Crate, schema: Option<WireSchema>, version: &str, access: Access) -> Package {
    assemble_with_license(krate, schema, version, DEFAULT_LICENSE, access)
}

pub fn assemble_with_license(
    krate: &Crate,
    schema: Option<WireSchema>,
    version: &str,
    license: &str,
    access: Access,
) -> Package {
    let mut pkg = emit(krate);
    if let Some(schema) = schema {
        pkg.files.push(TsFile {
            stem: "purecrate-wire".to_string(),
            source: emit_wire(krate, schema),
        });
        pkg.files.push(TsFile {
            stem: schema.package().to_string(),
            source: copied(schema.package(), &format!("packages/boundary-{}", schema.runtime_dep()), adapter_source(schema)),
        });
    }
    // With a schema the index also exports `parseJson`, which reads the
    // 64-bit integers the schemas take.
    if schema.is_some() {
        let index = pkg.files.iter_mut().find(|f| f.stem == "index").expect("emit writes an index");
        index.source.push_str("export { parseJson } from \"purecrate\";\n");
    }
    // What the index exports to callers stays whole, `Char` and `Uuid` with
    // all their methods; a caller's bundler drops what it does not use.
    let mut uses = trim::uses(pkg.files.iter().map(|f| f.source.as_str()));
    uses.extend(trim::exported(&pkg.files.iter().find(|f| f.stem == "index").expect("index").source));
    pkg.files.push(TsFile {
        stem: RUNTIME_STEM.to_string(),
        source: copied("purecrate", "packages/boundary", &trim::trim_closed(RUNTIME_SOURCE, &uses)),
    });
    // The sources above name the runtime and adapter as packages; here they
    // are files beside them.
    let mut local = vec![("purecrate".to_string(), RUNTIME_STEM.to_string())];
    if let Some(schema) = schema {
        local.push((schema.package().to_string(), schema.package().to_string()));
    }
    for file in &mut pkg.files {
        for (package, stem) in &local {
            file.source = file.source.replace(&format!("from \"{package}\";"), &format!("from \"./{stem}.ts\";"));
        }
        // Rewriting `from "purecrate"` to `from "./purecrate-runtime.ts"` can
        // push an import over the width; wrap again after the paths are final.
        // Copied runtime and adapters are hand-written and must not be rewrapped.
        let copied = file.stem == "purecrate-runtime"
            || (file.stem.starts_with("purecrate-") && file.stem != "purecrate-wire");
        if !copied && !file.stem.contains('.') {
            file.source = wrap_source(&file.source);
        }
    }
    let manifests = [
        ("package.json", package_json(krate.name.as_str(), version, license, schema, access)),
        ("tsconfig.json", tsconfig()),
        ("tsconfig.build.json", tsconfig_build()),
    ];
    for (i, (stem, source)) in manifests.into_iter().enumerate() {
        pkg.files.insert(i, TsFile { stem: stem.to_string(), source });
    }
    pkg
}

/// A copy of a hand-written file from `packages/`, marked as generated so
/// `build` may replace it, and with the MIT copyright and permission notice
/// the license requires in every copy.
fn copied(package: &str, dir: &str, source: &str) -> String {
    format!(
        "{HEADER}// `{package}` from purecrate-ts ({dir}), copied in at the generator's revision.\n\
         //\n\
         {MIT_NOTICE}\n\
         {source}"
    )
}

const MIT_NOTICE: &str = "\
// Copyright (c) 2026 Wataru Manji
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the \"Software\"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
";

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

/// The one dependency left is the schema library, which the consumer's
/// own code uses too.
fn package_json(name: &str, version: &str, license: &str, schema: Option<WireSchema>, access: Access) -> String {
    let kebab = purecrate_ir::to_kebab(name);
    let mut exports = vec![format!("    \".\": {}", entry("index"))];
    let mut peers = Vec::new();
    if let Some(schema) = schema {
        exports.push(format!("    \"./wire\": {}", entry("purecrate-wire")));
        peers.push(format!("    \"{}\": \"^{}\"", schema.runtime_dep(), schema.version()));
    }
    let private = match access {
        Access::Private => "  \"private\": true,\n",
        Access::Publishable => "",
    };
    let engines = if schema.is_some() {
        "  \"engines\": { \"node\": \">=21\" },\n"
    } else {
        ""
    };
    let peers = if peers.is_empty() {
        String::new()
    } else {
        format!("  \"peerDependencies\": {{\n{}\n  }},\n", peers.join(",\n"))
    };
    format!(
        "{{\n  \"name\": \"{kebab}\",\n  \"version\": \"{version}\",\n  \"license\": \"{license}\",\n{private}  \"type\": \"module\",\n  \"sideEffects\": false,\n{engines}  \"exports\": {{\n{exports}\n  }},\n  \"files\": [\"dist\", \"src\"],\n  \"scripts\": {{\n    \"build\": \"tsc -p tsconfig.build.json\",\n    \"prepack\": \"npm run build\"\n  }},\n{peers}  \"devDependencies\": {{\n    \"typescript\": \"{TYPESCRIPT_RANGE}\"\n  }}\n}}\n",
        exports = exports.join(",\n"),
    )
}

/// Strict, and also clean under the unused-binding and unreachable-code
/// checks a consumer may turn on for vendored sources. `erasableSyntaxOnly` and
/// `verbatimModuleSyntax` hold the sources to what type stripping (node,
/// bundlers) can run: no `enum`, `namespace`, or parameter properties, and
/// every type-only import marked `type`.
fn tsconfig() -> String {
    "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    \"noUnusedLocals\": true,\n    \"noUnusedParameters\": true,\n    \"allowUnreachableCode\": false,\n    \"erasableSyntaxOnly\": true,\n    \"verbatimModuleSyntax\": true,\n    \"target\": \"ES2022\",\n    \"module\": \"ES2022\",\n    \"moduleResolution\": \"bundler\",\n    \"allowImportingTsExtensions\": true,\n    \"skipLibCheck\": true,\n    \"noEmit\": true\n  },\n  \"include\": [\"src\"]\n}\n"
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
    fn the_runtime_and_the_adapter_are_copied_in() {
        // The counter's types with serde derives, so the wire module reads them.
        let mut krate = counter_example();
        for item in &mut krate.items {
            match item {
                purecrate_ir::Item::Struct(s) => s.serde = purecrate_ir::Serde { ser: true, de: true },
                purecrate_ir::Item::Enum(e) => e.serde = purecrate_ir::Serde { ser: true, de: true },
                _ => {}
            }
        }
        let pkg = assemble_with(&krate, Some(WireSchema::Zod));
        let file = |stem: &str| &pkg.files.iter().find(|f| f.stem == stem).unwrap_or_else(|| panic!("no {stem}")).source;
        assert!(file(RUNTIME_STEM).starts_with(HEADER) && file(RUNTIME_STEM).contains("export const Int = {"));
        assert!(file(RUNTIME_STEM).contains("Copyright (c) 2026 Wataru Manji"));
        assert!(file(RUNTIME_STEM).contains("Permission is hereby granted"));
        assert!(file("purecrate-zod").contains("from \"./purecrate-runtime.ts\";"));
        assert!(file("purecrate-zod").contains("Copyright (c) 2026 Wataru Manji"));
        assert!(file("purecrate-wire").contains("from \"./purecrate-zod.ts\";"));
        for f in &pkg.files {
            assert!(!f.source.contains("from \"purecrate"), "{} imports a purecrate package", f.stem);
        }
        let manifest = file("package.json");
        assert!(manifest.contains("\"peerDependencies\": {\n    \"zod\": \"^"), "{manifest}");
        assert!(manifest.contains("\"license\": \"MIT\""), "{manifest}");
        assert!(manifest.contains("\"sideEffects\": false"), "{manifest}");
        assert!(manifest.contains("\"engines\": { \"node\": \">=21\" }"), "{manifest}");
        assert!(!manifest.contains("purecrate\""), "{manifest}");
        let plain = assemble(&counter_example());
        let manifest = &plain.files.iter().find(|f| f.stem == "package.json").unwrap().source;
        assert!(!manifest.contains("peerDependencies"), "{manifest}");
        assert!(manifest.contains("\"sideEffects\": false"), "{manifest}");
        assert!(!manifest.contains("\"engines\""), "{manifest}");
    }

    #[test]
    fn the_license_is_copied_into_the_manifest() {
        let pkg = assemble_with_license(&counter_example(), None, "1.2.3", "Apache-2.0", Access::Publishable);
        let manifest = &pkg.files.iter().find(|f| f.stem == "package.json").unwrap().source;
        assert!(manifest.contains("\"license\": \"Apache-2.0\""), "{manifest}");
        assert!(!manifest.contains("\"private\""), "{manifest}");
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

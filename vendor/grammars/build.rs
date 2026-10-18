use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    println!("cargo:rerun-if-changed=parsers");
    println!("cargo:rerun-if-changed=queries");

    let parsers = sorted_dirs(&root.join("parsers"));
    for (name, dir) in &parsers {
        let mut c = cc::Build::new();
        c.include(dir)
            .opt_level(2)
            .warnings(false)
            .file(dir.join("parser.c"));
        match (dir.join("scanner.c"), dir.join("scanner.cc")) {
            (c_scanner, _) if c_scanner.exists() => {
                c.file(c_scanner);
            }
            (_, cpp_scanner) if cpp_scanner.exists() => {
                cc::Build::new()
                    .cpp(true)
                    .include(dir)
                    .opt_level(2)
                    .warnings(false)
                    .file(cpp_scanner)
                    .compile(&format!("grammar-{name}-scanner"));
            }
            _ => {}
        }
        c.compile(&format!("grammar-{name}"));
    }

    let mut out = String::new();
    out.push_str("unsafe extern \"C\" {\n");
    for (name, _) in &parsers {
        writeln!(out, "    fn {}() -> *const ();", symbol(name)).unwrap();
    }
    out.push_str("}\n\n/// The compiled-in parser for the grammar `name`.\n");
    out.push_str("pub fn language(name: &str) -> Option<LanguageFn> {\n    match name {\n");
    for (name, _) in &parsers {
        writeln!(
            out,
            "        {name:?} => Some(unsafe {{ LanguageFn::from_raw({}) }}),",
            symbol(name)
        )
        .unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n\n");
    out.push_str("/// The query file `file` of `language`, as helix's runtime folder held it.\n");
    out.push_str("pub fn query(language: &str, file: &str) -> Option<&'static str> {\n");
    out.push_str("    match (language, file) {\n");
    for (language, dir) in sorted_dirs(&root.join("queries")) {
        for (file, path) in sorted_files(&dir) {
            writeln!(
                out,
                "        ({language:?}, {file:?}) => Some(include_str!({:?})),",
                path.display().to_string()
            )
            .unwrap();
        }
    }
    out.push_str("        _ => None,\n    }\n}\n");
    let generated = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("grammars.rs");
    fs::write(generated, out).unwrap();
}

fn symbol(name: &str) -> String {
    format!("tree_sitter_{}", name.replace('-', "_"))
}

fn sorted_dirs(dir: &Path) -> Vec<(String, PathBuf)> {
    sorted(dir).filter(|(_, path)| path.is_dir()).collect()
}

fn sorted_files(dir: &Path) -> Vec<(String, PathBuf)> {
    sorted(dir).filter(|(_, path)| path.is_file()).collect()
}

fn sorted(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    let mut entries: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name().into_string().unwrap(), entry.path())
        })
        .collect();
    entries.sort();
    entries.into_iter()
}

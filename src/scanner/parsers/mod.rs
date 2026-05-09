//! Language adapters: each adapter owns parse + resolve logic for one language.
//!
//! # Adding a new language (e.g., Lua)
//!
//! 1. Create `src/scanner/parsers/lua.rs` implementing `LanguageParser`.
//! 2. Add one line to `registry()` in this file: `("lua", Box::new(LuaParser))`.
//!
//! That's it — no other files need to change.

pub mod csharp;
pub mod dart;
pub mod go;
pub mod haskell;
pub mod java;
pub mod javascript;
pub mod kotlin;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod typescript;
pub mod zig;

/// A single import statement found in a source file.
pub struct ImportEntry {
    pub path: String,
    pub line_number: i32,
}

/// Common interface for all language adapters.
///
/// # Contract
/// - `parse` must return every import path present in `source`, in source order.
/// - `resolve` must return `Some(project_relative_path)` when the import refers to
///   a file that exists in `known_paths`, and `None` for external/stdlib dependencies.
///
/// Both methods are pure functions — no mutable state, no I/O.
pub trait LanguageParser: Send + Sync {
    /// Extract import entries from the given source text.
    fn parse(&self, source: &str) -> Vec<ImportEntry>;

    /// Resolve one import path to a project-relative file path.
    ///
    /// `source_file` is the project-relative path of the file that contains the
    /// import (needed for relative-path languages like Rust `crate::`, TypeScript
    /// `./foo`, Python `.bar`, Zig `utils.zig`, etc.).
    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String>;
}

/// Maps each supported file extension to its language adapter.
///
/// Extensions that share an adapter (e.g., `js`/`jsx` both use `JavaScriptParser`)
/// appear as separate entries pointing to separate (but behaviourally identical) boxes.
pub fn registry() -> Vec<(&'static str, Box<dyn LanguageParser>)> {
    vec![
        ("rs", Box::new(rust::RustParser)),
        ("kt", Box::new(kotlin::KotlinParser)),
        ("kts", Box::new(kotlin::KotlinParser)),
        ("ts", Box::new(typescript::TypeScriptParser)),
        ("tsx", Box::new(typescript::TsxParser)),
        ("swift", Box::new(swift::SwiftParser)),
        ("cs", Box::new(csharp::CSharpParser)),
        ("go", Box::new(go::GoParser)),
        ("hs", Box::new(haskell::HaskellParser)),
        ("java", Box::new(java::JavaParser)),
        ("js", Box::new(javascript::JavaScriptParser)),
        ("jsx", Box::new(javascript::JavaScriptParser)),
        ("py", Box::new(python::PythonParser)),
        ("dart", Box::new(dart::DartParser)),
        ("php", Box::new(php::PhpParser)),
        ("rb", Box::new(ruby::RubyParser)),
        ("zig", Box::new(zig::ZigParser)),
    ]
}

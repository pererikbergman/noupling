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
pub mod elixir;
pub mod go;
pub mod haskell;
pub mod java;
pub mod javascript;
pub mod kotlin;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod swift;
pub mod typescript;
pub mod zig;

/// A single import statement found in a source file.
pub struct ImportEntry {
    pub path: String,
    pub line_number: i32,
}

/// Per-file count of abstract vs concrete type declarations.
///
/// Abstract = trait / interface / abstract class. Concrete = everything else
/// that declares a named type (struct, enum, class, etc.). Used to compute the
/// Martin abstractness metric `A = abstract / (abstract + concrete)` per directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeCounts {
    pub abstract_count: usize,
    pub concrete_count: usize,
}

/// True when `path` ends with `candidate` on a `/`-separated segment boundary.
///
/// `"foo/bar.py".ends_with("bar.py")` is true under both this helper and the
/// stdlib `ends_with`. The difference: `"structure.py".ends_with("re.py")` is
/// true via stdlib but false here — a path suffix has to start at a segment
/// boundary, not at an arbitrary byte. This prevents stdlib imports like
/// `import re` from substring-matching unrelated project files.
pub fn ends_with_segment(path: &str, candidate: &str) -> bool {
    path == candidate || path.ends_with(&format!("/{}", candidate))
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

    /// Count abstract vs concrete type declarations in the source.
    ///
    /// Default returns zeros; languages that participate in the abstractness
    /// metric override this. See `TypeCounts`.
    fn count_type_declarations(&self, _source: &str) -> TypeCounts {
        TypeCounts::default()
    }
}

#[cfg(test)]
mod tests {
    use super::ends_with_segment;

    #[test]
    fn ends_with_segment_matches_exact_path() {
        assert!(ends_with_segment("foo.py", "foo.py"));
    }

    #[test]
    fn ends_with_segment_matches_slash_boundary() {
        assert!(ends_with_segment("src/pkg/foo.py", "pkg/foo.py"));
        assert!(ends_with_segment("src/pkg/foo.py", "foo.py"));
    }

    #[test]
    fn ends_with_segment_rejects_mid_segment_suffix() {
        assert!(!ends_with_segment("src/structure.py", "re.py"));
        assert!(!ends_with_segment("src/chaos.py", "os.py"));
        assert!(!ends_with_segment("src/figure.py", "re.py"));
    }

    #[test]
    fn ends_with_segment_rejects_partial_segment_match() {
        assert!(!ends_with_segment("xfoo/bar.py", "foo/bar.py"));
    }
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
        ("ex", Box::new(elixir::ElixirParser)),
        ("exs", Box::new(elixir::ElixirParser)),
        ("scala", Box::new(scala::ScalaParser)),
        ("sc", Box::new(scala::ScalaParser)),
    ]
}

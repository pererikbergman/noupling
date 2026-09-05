//! ReportFormatter trait + registry. Originally (#301) the seam
//! covered only the six "string + write + print" simple formats.
//! Issue #317 widens it so the directory-emitting formats (md, html)
//! and the deps-aware single-file formats (bundle, dashboard, pr) all
//! register as adapters too. `commands/report.rs` is now thin
//! dispatch: every format except `strategy` and `explorer` (which
//! need the `DatabaseSession` and a much wider option surface) flows
//! through here.
//!
//! Output is no longer a bare `(path, content)` pair — directory
//! adapters write their own files. The dispatcher's job became
//! "ask the adapter to produce an `Output` and then handle the
//! tail."

use anyhow::Result;
use noupling_core::analyzer::AuditResult;
use noupling_core::core::{Dependency, Module, Snapshot};
use noupling_core::settings::Settings;
use std::path::{Path, PathBuf};

/// Inputs an adapter may need. Wider than #301 because directory
/// adapters (md/html) want `settings`, and the deps-aware adapters
/// (bundle/dashboard) want the dependency list. `prev_score` and
/// `prev_violation_count` carry the delta inputs for the `pr`
/// adapter; both are `None` when there's no history.
///
/// Adapters use what they need and ignore the rest — that's
/// honest: the cost of "extra fields the adapter doesn't read" is
/// far smaller than the cost of an N-of-12 dispatch fork in
/// commands/report.rs.
pub struct FormatterContext<'a> {
    pub modules: &'a [Module],
    pub deps: &'a [Dependency],
    pub result: &'a AuditResult,
    pub snapshot: &'a Snapshot,
    pub report_dir: &'a Path,
    pub settings: &'a Settings,
    pub prev_score: Option<f64>,
    pub prev_violation_count: Option<usize>,
}

/// What an adapter produces.
///
/// `SingleFile` is the common case — the dispatcher writes the bytes
/// and prints the tail.
///
/// `Directory` means the adapter wrote its own files (md/html each
/// emit a tree of pages); the dispatcher prints the success line and
/// nothing else.
pub enum Output {
    SingleFile {
        file_path: PathBuf,
        content: String,
        success_tail: Option<String>,
    },
    Directory {
        path: PathBuf,
        success_tail: Option<String>,
    },
}

/// The seam. One method, one outcome.
pub trait ReportFormatter {
    fn name(&self) -> &'static str;
    fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output>;
}

/// Built-in adapters — the fourteen formats minus `strategy` and
/// `explorer`, which keep bespoke arms in `commands/report.rs`.
/// Strategy needs the `SnapshotRepository`/`ModuleRepository`/
/// `DependencyRepository` triad for its history walk; explorer
/// carries an option struct (editor, title, no-history, override
/// output path, LLM enrichment, auto-detected layers, …) that
/// would balloon `FormatterContext` for no gain.
pub fn builtin_formatters() -> Vec<Box<dyn ReportFormatter>> {
    vec![
        Box::new(adapters::Text),
        Box::new(adapters::Json),
        Box::new(adapters::Xml),
        Box::new(adapters::Sonar),
        Box::new(adapters::Mermaid),
        Box::new(adapters::Dot),
        Box::new(adapters::Briefing),
        Box::new(adapters::Markdown),
        Box::new(adapters::Html),
        Box::new(adapters::Bundle),
        Box::new(adapters::Dashboard),
        Box::new(adapters::Pr),
    ]
}

/// Dispatch a format string against the registry. Returns the
/// rendered output when one of the adapters matches; `None` lets the
/// caller fall through to the remaining bespoke arms (strategy,
/// explorer, all).
pub fn dispatch<'a>(
    format: &str,
    ctx: &FormatterContext<'a>,
    registry: &[Box<dyn ReportFormatter>],
) -> Result<Option<Output>> {
    for adapter in registry {
        if adapter.name() == format {
            return adapter.render(ctx).map(Some);
        }
    }
    Ok(None)
}

/// Write the adapter's output and print the success line. Single
/// entry point so every caller (`commands::report::run` for focused
/// formats, the `all` arm for batch emit) does the same thing.
pub fn write(out: &Output) -> Result<()> {
    match out {
        Output::SingleFile {
            file_path,
            content,
            success_tail,
        } => {
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(file_path, content)?;
            println!("Report saved to {}", file_path.display());
            if let Some(tail) = success_tail {
                println!("{}", tail);
            }
        }
        Output::Directory { path, success_tail } => {
            println!("Report saved to {}", path.display());
            if let Some(tail) = success_tail {
                println!("{}", tail);
            }
        }
    }
    Ok(())
}

mod adapters {
    use super::*;
    use crate::reporter;

    /// The same text `audit` prints, written to `.noupling/report.txt`
    /// so `report --format text --baseline` and `report --format all`
    /// carry it (#343).
    pub struct Text;
    impl ReportFormatter for Text {
        fn name(&self) -> &'static str {
            "text"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            Ok(Output::SingleFile {
                file_path: ctx.report_dir.join("report.txt"),
                content: reporter::format_text(ctx.result),
                success_tail: None,
            })
        }
    }

    pub struct Json;
    impl ReportFormatter for Json {
        fn name(&self) -> &'static str {
            "json"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let report =
                reporter::JsonReport::from_audit(ctx.modules, ctx.result, &ctx.snapshot.id);
            Ok(Output::SingleFile {
                file_path: ctx.report_dir.join("report.json"),
                content: report.to_json()?,
                success_tail: None,
            })
        }
    }

    pub struct Xml;
    impl ReportFormatter for Xml {
        fn name(&self) -> &'static str {
            "xml"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            Ok(Output::SingleFile {
                file_path: ctx.report_dir.join("report.xml"),
                content: reporter::format_xml(ctx.modules, ctx.result, &ctx.snapshot.id),
                success_tail: None,
            })
        }
    }

    pub struct Sonar;
    impl ReportFormatter for Sonar {
        fn name(&self) -> &'static str {
            "sonar"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let file_path = ctx.report_dir.join("noupling-sonar.json");
            let tail = format!(
                "Add to sonar-project.properties: sonar.externalIssuesReportPaths={}",
                file_path.display()
            );
            Ok(Output::SingleFile {
                file_path,
                content: reporter::format_sonar(ctx.modules, ctx.result),
                success_tail: Some(tail),
            })
        }
    }

    pub struct Mermaid;
    impl ReportFormatter for Mermaid {
        fn name(&self) -> &'static str {
            "mermaid"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            Ok(Output::SingleFile {
                file_path: ctx.report_dir.join("report.mermaid"),
                content: reporter::format_mermaid(ctx.modules, ctx.result),
                success_tail: None,
            })
        }
    }

    pub struct Dot;
    impl ReportFormatter for Dot {
        fn name(&self) -> &'static str {
            "dot"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let file_path = ctx.report_dir.join("report.dot");
            let tail = format!(
                "Render with: dot -Tpng {} -o graph.png",
                file_path.display()
            );
            Ok(Output::SingleFile {
                file_path,
                content: reporter::format_dot(ctx.modules, ctx.result),
                success_tail: Some(tail),
            })
        }
    }

    pub struct Briefing;
    impl ReportFormatter for Briefing {
        fn name(&self) -> &'static str {
            "briefing"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            Ok(Output::SingleFile {
                file_path: ctx.report_dir.join("briefing.md"),
                content: reporter::format_briefing(ctx.result),
                success_tail: None,
            })
        }
    }

    pub struct Markdown;
    impl ReportFormatter for Markdown {
        fn name(&self) -> &'static str {
            "md"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let md_dir = ctx.report_dir.join("report-md");
            reporter::generate_markdown_report(ctx.modules, ctx.result, &ctx.snapshot.id, &md_dir)?;
            // The previous bespoke arm printed `…/report-md/README.md`;
            // we render the same path so the user sees no diff.
            Ok(Output::Directory {
                path: md_dir.join("README.md"),
                success_tail: None,
            })
        }
    }

    pub struct Html;
    impl ReportFormatter for Html {
        fn name(&self) -> &'static str {
            "html"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let html_dir = ctx.report_dir.join("report");
            reporter::generate_html_report(
                ctx.modules,
                ctx.result,
                &ctx.snapshot.id,
                &html_dir,
                ctx.settings,
            )?;
            Ok(Output::Directory {
                path: html_dir.join("index.html"),
                success_tail: None,
            })
        }
    }

    pub struct Bundle;
    impl ReportFormatter for Bundle {
        fn name(&self) -> &'static str {
            "bundle"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let file_path = ctx.report_dir.join("bundle.html");
            reporter::generate_bundle_report(ctx.modules, ctx.deps, ctx.result, &file_path)?;
            // Bundle writes its own bytes; surface as Directory so
            // we don't double-write.
            Ok(Output::Directory {
                path: file_path,
                success_tail: None,
            })
        }
    }

    pub struct Dashboard;
    impl ReportFormatter for Dashboard {
        fn name(&self) -> &'static str {
            "dashboard"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let file_path = ctx.report_dir.join("dashboard.html");
            reporter::generate_dashboard(ctx.modules, ctx.deps, ctx.result, &file_path)?;
            Ok(Output::Directory {
                path: file_path,
                success_tail: None,
            })
        }
    }

    pub struct Pr;
    impl ReportFormatter for Pr {
        fn name(&self) -> &'static str {
            "pr"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<Output> {
            let content = reporter::format_pr(
                ctx.result,
                ctx.prev_score,
                ctx.prev_violation_count,
                None,
                None,
            );
            Ok(Output::SingleFile {
                file_path: ctx.report_dir.join("pr.md"),
                content,
                success_tail: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::analyzer::AuditResultBuilder;
    use noupling_core::core::{Module, ModuleType, Snapshot};
    use tempfile::TempDir;

    struct Scaffold {
        _tmp: TempDir,
        report_dir: PathBuf,
        modules: Vec<Module>,
        deps: Vec<Dependency>,
        result: AuditResult,
        snapshot: Snapshot,
        settings: Settings,
    }

    fn scaffold() -> Scaffold {
        let tmp = tempfile::tempdir().expect("tempdir");
        let report_dir = tmp.path().to_path_buf();
        let modules = vec![Module {
            id: "m-1".into(),
            snapshot_id: "snap-1".into(),
            parent_id: None,
            name: "main.rs".into(),
            path: "src/main.rs".into(),
            module_type: ModuleType::File,
            depth: 1,
        }];
        let result = AuditResultBuilder::new().build();
        let snapshot = Snapshot {
            id: "snap-1".into(),
            timestamp: "2026-06-05T00:00:00".into(),
            root_path: tmp.path().to_string_lossy().to_string(),
        };
        let settings: Settings = serde_json::from_str("{}").expect("default settings");
        Scaffold {
            _tmp: tmp,
            report_dir,
            modules,
            deps: Vec::new(),
            result,
            snapshot,
            settings,
        }
    }

    fn ctx_of(s: &Scaffold) -> FormatterContext<'_> {
        FormatterContext {
            modules: &s.modules,
            deps: &s.deps,
            result: &s.result,
            snapshot: &s.snapshot,
            report_dir: &s.report_dir,
            settings: &s.settings,
            prev_score: None,
            prev_violation_count: None,
        }
    }

    #[test]
    fn registry_includes_every_non_bespoke_format() {
        let names: Vec<&'static str> = builtin_formatters().iter().map(|f| f.name()).collect();
        for expected in [
            "json",
            "xml",
            "sonar",
            "mermaid",
            "dot",
            "briefing",
            "md",
            "html",
            "bundle",
            "dashboard",
            "pr",
        ] {
            assert!(
                names.contains(&expected),
                "missing {expected} in {:?}",
                names
            );
        }
    }

    #[test]
    fn dispatch_returns_none_when_format_is_unknown() {
        let s = scaffold();
        let ctx = ctx_of(&s);
        let out = dispatch("explorer", &ctx, &builtin_formatters()).expect("ok");
        assert!(out.is_none(), "explorer is still bespoke");
        let out = dispatch("strategy", &ctx, &builtin_formatters()).expect("ok");
        assert!(out.is_none(), "strategy is still bespoke");
    }

    #[test]
    fn dispatch_renders_json_as_single_file_with_default_name() {
        let s = scaffold();
        let ctx = ctx_of(&s);
        let out = dispatch("json", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("json matched");
        match out {
            Output::SingleFile {
                file_path,
                content,
                success_tail,
            } => {
                assert!(file_path.ends_with("report.json"));
                assert!(content.starts_with("{"), "got: {}", content);
                assert!(success_tail.is_none());
            }
            Output::Directory { .. } => panic!("json should be SingleFile"),
        }
    }

    #[test]
    fn sonar_carries_the_sonar_project_properties_hint_as_a_tail() {
        let s = scaffold();
        let ctx = ctx_of(&s);
        let out = dispatch("sonar", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("sonar matched");
        match out {
            Output::SingleFile { success_tail, .. } => {
                let tail = success_tail.expect("sonar tail set");
                assert!(tail.contains("sonar.externalIssuesReportPaths"));
            }
            Output::Directory { .. } => panic!("sonar should be SingleFile"),
        }
    }

    #[test]
    fn dot_carries_the_render_hint_as_a_tail() {
        let s = scaffold();
        let ctx = ctx_of(&s);
        let out = dispatch("dot", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("dot matched");
        match out {
            Output::SingleFile { success_tail, .. } => {
                let tail = success_tail.expect("dot tail set");
                assert!(tail.contains("dot -Tpng"));
            }
            Output::Directory { .. } => panic!("dot should be SingleFile"),
        }
    }

    #[test]
    fn md_adapter_announces_a_directory_output_under_report_md() {
        let s = scaffold();
        let ctx = ctx_of(&s);
        let out = dispatch("md", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("md matched");
        match out {
            Output::Directory { path, .. } => {
                assert!(path.to_string_lossy().contains("report-md"));
                assert!(path.ends_with("README.md"));
            }
            Output::SingleFile { .. } => panic!("md should be Directory"),
        }
    }

    #[test]
    fn pr_adapter_consumes_prev_score_when_present() {
        let s = scaffold();
        let mut ctx = ctx_of(&s);
        ctx.prev_score = Some(80.0);
        ctx.prev_violation_count = Some(3);
        let out = dispatch("pr", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("pr matched");
        match out {
            Output::SingleFile { content, .. } => {
                assert!(content.contains("Score"));
                assert!(content.contains("since previous") || content.contains("(+"));
            }
            Output::Directory { .. } => panic!("pr should be SingleFile"),
        }
    }
}

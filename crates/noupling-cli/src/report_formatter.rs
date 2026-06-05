//! ReportFormatter trait + registry (#301). Eight of the report-format
//! arms used to repeat the same three-line pattern in `report.rs`:
//! compute string, write to `report_dir/file.ext`, print
//! `"Report saved to …"`. This pulls that into one seam — each adapter
//! is a tiny module owning *which file name and what bytes*, and the
//! dispatch is one registry lookup.
//!
//! The format families that don't fit this shape (Explorer, PR, HTML,
//! Markdown, Bundle, Dashboard, Strategy, the composite "all") stay
//! as bespoke match arms in `report.rs`. They have different inputs —
//! a second adapter would justify expanding the seam to cover them.
//! One adapter today = hypothetical seam.

use anyhow::Result;
use noupling_core::analyzer::AuditResult;
use noupling_core::core::{Module, Snapshot};
use std::path::{Path, PathBuf};

/// Inputs an adapter needs to render. Kept narrow — anything an
/// adapter doesn't use lives outside this struct so the seam stays
/// honest.
pub struct FormatterContext<'a> {
    pub modules: &'a [Module],
    pub result: &'a AuditResult,
    pub snapshot: &'a Snapshot,
    pub report_dir: &'a Path,
}

/// What an adapter returns. The caller writes the file and prints the
/// (optional) success-tail message uniformly — the format-specific
/// "Render with: dot …" / "Add to sonar-project.properties…" lines
/// belong to the format, not the dispatcher.
pub struct FormattedReport {
    pub file_path: PathBuf,
    pub content: String,
    pub success_tail: Option<String>,
}

/// The seam. One method, one outcome.
pub trait ReportFormatter {
    fn name(&self) -> &'static str;
    fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport>;
}

/// Built-in adapters — the eight string-and-file simple formats.
pub fn builtin_formatters() -> Vec<Box<dyn ReportFormatter>> {
    vec![
        Box::new(adapters::Json),
        Box::new(adapters::Xml),
        Box::new(adapters::Sonar),
        Box::new(adapters::Mermaid),
        Box::new(adapters::Dot),
        Box::new(adapters::Briefing),
    ]
}

/// Dispatch a format string against the registry. Returns the rendered
/// report when one of the adapters matches; `None` lets the caller
/// fall through to its bespoke match arms.
pub fn dispatch<'a>(
    format: &str,
    ctx: &FormatterContext<'a>,
    registry: &[Box<dyn ReportFormatter>],
) -> Result<Option<FormattedReport>> {
    for adapter in registry {
        if adapter.name() == format {
            return adapter.render(ctx).map(Some);
        }
    }
    Ok(None)
}

mod adapters {
    use super::*;
    use crate::reporter;

    pub struct Json;
    impl ReportFormatter for Json {
        fn name(&self) -> &'static str {
            "json"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport> {
            let report =
                reporter::JsonReport::from_audit(ctx.modules, ctx.result, &ctx.snapshot.id);
            Ok(FormattedReport {
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
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport> {
            Ok(FormattedReport {
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
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport> {
            let file_path = ctx.report_dir.join("noupling-sonar.json");
            let tail = format!(
                "Add to sonar-project.properties: sonar.externalIssuesReportPaths={}",
                file_path.display()
            );
            Ok(FormattedReport {
                file_path,
                content: reporter::format_sonar(ctx.result),
                success_tail: Some(tail),
            })
        }
    }

    pub struct Mermaid;
    impl ReportFormatter for Mermaid {
        fn name(&self) -> &'static str {
            "mermaid"
        }
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport> {
            Ok(FormattedReport {
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
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport> {
            let file_path = ctx.report_dir.join("report.dot");
            let tail = format!(
                "Render with: dot -Tpng {} -o graph.png",
                file_path.display()
            );
            Ok(FormattedReport {
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
        fn render(&self, ctx: &FormatterContext<'_>) -> Result<FormattedReport> {
            Ok(FormattedReport {
                file_path: ctx.report_dir.join("briefing.md"),
                content: reporter::format_briefing(ctx.result),
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

    /// Tiny scaffolding for each adapter test: one snapshot, one
    /// module, a default audit result. Returned by-value so the test
    /// can take references off them with normal borrow rules.
    struct Scaffold {
        _tmp: TempDir,
        report_dir: PathBuf,
        modules: Vec<Module>,
        result: AuditResult,
        snapshot: Snapshot,
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
        Scaffold {
            _tmp: tmp,
            report_dir,
            modules,
            result,
            snapshot,
        }
    }

    #[test]
    fn registry_includes_all_six_simple_formats() {
        let names: Vec<&'static str> = builtin_formatters().iter().map(|f| f.name()).collect();
        assert!(names.contains(&"json"));
        assert!(names.contains(&"xml"));
        assert!(names.contains(&"sonar"));
        assert!(names.contains(&"mermaid"));
        assert!(names.contains(&"dot"));
        assert!(names.contains(&"briefing"));
    }

    #[test]
    fn dispatch_returns_none_when_format_is_unknown() {
        let s = scaffold();
        let ctx = FormatterContext {
            modules: &s.modules,
            result: &s.result,
            snapshot: &s.snapshot,
            report_dir: &s.report_dir,
        };
        let out = dispatch("explorer", &ctx, &builtin_formatters()).expect("ok");
        assert!(
            out.is_none(),
            "explorer doesn't live in the simple registry"
        );
    }

    #[test]
    fn dispatch_renders_json_with_default_file_name() {
        let s = scaffold();
        let ctx = FormatterContext {
            modules: &s.modules,
            result: &s.result,
            snapshot: &s.snapshot,
            report_dir: &s.report_dir,
        };
        let out = dispatch("json", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("json adapter matched");
        assert!(out.file_path.ends_with("report.json"));
        assert!(out.content.starts_with("{"), "got: {}", out.content);
        assert!(out.success_tail.is_none());
    }

    #[test]
    fn sonar_carries_the_sonar_project_properties_hint_as_a_tail() {
        let s = scaffold();
        let ctx = FormatterContext {
            modules: &s.modules,
            result: &s.result,
            snapshot: &s.snapshot,
            report_dir: &s.report_dir,
        };
        let out = dispatch("sonar", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("sonar matched");
        let tail = out.success_tail.expect("sonar tail set");
        assert!(tail.contains("sonar.externalIssuesReportPaths"));
    }

    #[test]
    fn dot_carries_the_render_hint_as_a_tail() {
        let s = scaffold();
        let ctx = FormatterContext {
            modules: &s.modules,
            result: &s.result,
            snapshot: &s.snapshot,
            report_dir: &s.report_dir,
        };
        let out = dispatch("dot", &ctx, &builtin_formatters())
            .expect("ok")
            .expect("dot matched");
        let tail = out.success_tail.expect("dot tail set");
        assert!(tail.contains("dot -Tpng"));
    }
}

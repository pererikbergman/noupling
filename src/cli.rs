use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "noupling",
    about = "Architecture auditing CLI that detects coupling violations and circular dependencies in source code.",
    long_about = "\
noupling scans source code projects, extracts import/dependency relationships \
using Tree-sitter, and analyzes the architectural health by detecting:

  - Coupling violations: sibling modules that depend on each other
  - Circular dependencies: dependency chains that form loops (A -> B -> C -> A)

SUPPORTED LANGUAGES:
  C# (.cs), Go (.go), Haskell (.hs), Java (.java), JavaScript (.js, .jsx),
  Kotlin (.kt, .kts), Python (.py), Rust (.rs), Swift (.swift),
  TypeScript (.ts, .tsx), Zig (.zig)

TYPICAL WORKFLOW:
  1. noupling scan <PATH>                   Scan project and store results
  2. noupling audit <PATH>                  View health score and violations
  3. noupling report <PATH> --format html   Generate navigable HTML report

DATA STORAGE:
  All data is stored in <PATH>/.noupling/ including:
  - history.db       SQLite database with snapshots, modules, and dependencies
  - settings.json    Configurable thresholds and ignore patterns (auto-created)
  - report.*         Generated report files

CONFIGURATION (.noupling/settings.json):
  {
    \"thresholds\": {
      \"score_green\": 90.0,       // Score >= this is green (healthy)
      \"score_yellow\": 70.0,      // Score >= this is yellow (warning)
      \"critical_severity\": 0.5,  // Violations above this are critical
      \"minimum_severity\": 0.2    // Hide violations below this threshold
    },
    \"ignore_patterns\": [          // Glob patterns (gitignore-style)
      \"**/build/**\",
      \"**/generated/**\"
    ],
    \"source_extensions\": [\"kt\", \"java\", \"ts\", \"rs\"]  // File types to scan
  }

HEALTH SCORE:
  Score = 100 * (1 - sum_of_severities / total_modules)
  Coupling severity = 1 / (depth + 1)    (deeper = less severe)
  Circular severity = modules / (depth + 1) / 10   (always significant)

EXAMPLES:
  noupling init .                           Create default settings.json
  noupling scan /path/to/android/app        Scan an Android project
  noupling audit /path/to/android/app       Show health score and violations
  noupling report . --format json           JSON report (comprehensive)
  noupling report . --format xml            XML report (comprehensive)
  noupling report . --format md             Multi-file Markdown report
  noupling report . --format html           Interactive HTML report with drill-down
  noupling report . --format sonar          SonarCloud generic issue import format",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create .noupling/settings.json with default thresholds, ignore patterns, and extensions.
    /// Settings are auto-created on first run of any command, but init lets you customize before scanning.
    Init {
        /// Path to the project root
        #[arg(default_value = ".")]
        path: String,
    },

    /// Install or uninstall a git pre-commit hook that runs noupling before each commit.
    /// The hook scans changed files and fails the commit if new violations are introduced.
    /// Bypass with `git commit --no-verify`.
    Hook {
        /// Action: "install" or "uninstall"
        action: String,

        /// Path to the project root
        #[arg(default_value = ".")]
        path: String,
    },

    /// Scan a project directory: discover source files, parse imports via Tree-sitter,
    /// resolve dependencies, and store results in .noupling/history.db.
    /// Each scan creates a new snapshot with a unique ID.
    Scan {
        /// Path to the project root to scan
        path: String,

        /// Only report violations involving files changed compared to this branch.
        /// Uses git diff to detect changes. Scans the full project for dependency
        /// resolution but filters results to changed files only.
        /// Example: --diff-base main, --diff-base origin/develop
        #[arg(long)]
        diff_base: Option<String>,
    },

    /// Save or manage the violation baseline.
    /// `noupling baseline save .` saves current violations as the accepted baseline.
    /// Future audits with --baseline will only report NEW violations not in the baseline.
    Baseline {
        /// Action: "save" to create baseline from current violations
        action: String,

        /// Path to the project root
        #[arg(default_value = ".")]
        path: String,
    },

    /// Run the coupling and circular dependency analysis on the latest (or specified) snapshot.
    /// Displays health score, violation count, and detailed violation list to stdout.
    Audit {
        /// Path to the project root (reads from .noupling/history.db)
        #[arg(default_value = ".")]
        path: String,

        /// Audit a specific snapshot by ID instead of the latest one
        #[arg(long)]
        snapshot: Option<String>,

        /// Exit with code 1 if health score is below this threshold.
        /// Use in CI to gate merges: --fail-below 80
        #[arg(long)]
        fail_below: Option<f64>,

        /// Compare against the saved baseline. Only report NEW violations
        /// not in .noupling/baseline.json. Exit code 1 only on new violations.
        #[arg(long)]
        baseline: bool,

        /// Show results for a specific module only (monorepo mode).
        /// Module names are defined in .noupling/settings.json under "modules".
        #[arg(long, value_name = "NAME")]
        module: Option<String>,
    },

    /// Show health score history across snapshots. Track architectural drift over time.
    Trend {
        /// Path to the project root (reads from .noupling/history.db)
        #[arg(default_value = ".")]
        path: String,

        /// Show only the last N snapshots
        #[arg(long, default_value = "20")]
        last: usize,

        /// Show per-module score trends instead of overall score
        #[arg(long)]
        by_module: bool,
    },

    /// Generate a report file from the latest snapshot's audit results.
    /// The report is saved to .noupling/ (or .noupling/report/ for html/md).
    Report {
        /// Path to the project root (reads from .noupling/history.db)
        #[arg(default_value = ".")]
        path: String,

        /// Output format: json, xml, md, html, sonar, mermaid, dot, bundle, dashboard, pr, or all.
        ///
        /// json      - Comprehensive JSON with directory tree, grouped cycles, and coupling details.
        /// xml       - Same structure as JSON but in XML format.
        /// md        - Multi-file Markdown with navigable README.md per directory.
        /// html      - Interactive static HTML with drill-down navigation and color-coded scores.
        /// sonar     - SonarCloud/SonarQube generic issue import format.
        /// mermaid   - Mermaid flowchart diagram of dependencies.
        /// dot       - GraphViz DOT graph for custom rendering.
        /// bundle    - Zoomable sunburst with dependency edges (D3.js).
        /// dashboard - Interactive technical leader dashboard (D3.js).
        /// pr        - Tight Markdown summary for posting as a PR comment.
        /// all       - Generate every format above into .noupling/ in one command.
        #[arg(long)]
        format: String,

        /// Generate report for a specific module only (monorepo mode).
        /// Module names are defined in .noupling/settings.json under "modules".
        #[arg(long, value_name = "NAME")]
        module: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_scan_command() {
        let cli = Cli::parse_from(["noupling", "scan", "/path/to/project"]);
        match cli.command {
            Commands::Scan { path, diff_base } => {
                assert_eq!(path, "/path/to/project");
                assert!(diff_base.is_none());
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn parse_audit_command_no_snapshot() {
        let cli = Cli::parse_from(["noupling", "audit"]);
        match cli.command {
            Commands::Audit {
                snapshot,
                path,
                fail_below,
                baseline,
                ..
            } => {
                assert!(snapshot.is_none());
                assert_eq!(path, ".");
                assert!(!baseline);
                assert!(fail_below.is_none());
            }
            _ => panic!("Expected Audit command"),
        }
    }

    #[test]
    fn parse_audit_command_with_snapshot() {
        let cli = Cli::parse_from(["noupling", "audit", "--snapshot", "abc-123"]);
        match cli.command {
            Commands::Audit { snapshot, .. } => assert_eq!(snapshot, Some("abc-123".to_string())),
            _ => panic!("Expected Audit command"),
        }
    }

    #[test]
    fn parse_audit_with_fail_below() {
        let cli = Cli::parse_from(["noupling", "audit", "--fail-below", "80"]);
        match cli.command {
            Commands::Audit { fail_below, .. } => assert_eq!(fail_below, Some(80.0)),
            _ => panic!("Expected Audit command"),
        }
    }

    #[test]
    fn parse_audit_with_path() {
        let cli = Cli::parse_from(["noupling", "audit", "examples/kotlin-clean"]);
        match cli.command {
            Commands::Audit { path, .. } => assert_eq!(path, "examples/kotlin-clean"),
            _ => panic!("Expected Audit command"),
        }
    }

    #[test]
    fn parse_report_json() {
        let cli = Cli::parse_from(["noupling", "report", "--format", "json"]);
        match cli.command {
            Commands::Report { format, path, .. } => {
                assert_eq!(format, "json");
                assert_eq!(path, ".");
            }
            _ => panic!("Expected Report command"),
        }
    }

    #[test]
    fn parse_report_md() {
        let cli = Cli::parse_from(["noupling", "report", "--format", "md"]);
        match cli.command {
            Commands::Report { format, .. } => assert_eq!(format, "md"),
            _ => panic!("Expected Report command"),
        }
    }
}

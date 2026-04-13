use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "noupling", about = "Architecture auditing tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan a project directory for dependencies
    Scan {
        /// Path to the project root
        path: String,
    },
    /// Audit the dependency graph for coupling violations
    Audit {
        /// Path to the project root (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Specific snapshot ID to audit (defaults to latest)
        #[arg(long)]
        snapshot: Option<String>,
    },
    /// Generate a report from the latest audit
    Report {
        /// Path to the project root (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Output format: json or md
        #[arg(long)]
        format: String,
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
            Commands::Scan { path } => assert_eq!(path, "/path/to/project"),
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn parse_audit_command_no_snapshot() {
        let cli = Cli::parse_from(["noupling", "audit"]);
        match cli.command {
            Commands::Audit { snapshot, path } => {
                assert!(snapshot.is_none());
                assert_eq!(path, ".");
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
            Commands::Report { format, path } => {
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

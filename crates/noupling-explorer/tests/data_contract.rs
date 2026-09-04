//! Integration tests for the Explorer's Data Contract serialization.
//!
//! Each test exercises the public `render()` API and inspects the JSON
//! injected into the placeholder `<script id="noupling-data">` block.
//!
//! These tests are the spec for the Data Contract documented in
//! `docs/noupling-explorer-prd.md` §6 — if these pass, downstream
//! template authors can rely on the shape.

use noupling_core::analyzer::{
    AuditResultBuilder, CouplingViolation, DependencyDirection, GravityWell, RedFlag, RedFlagType,
};
use noupling_core::core::{Dependency, Module, ModuleType, Snapshot};
use noupling_core::settings::Settings;
use noupling_explorer::{render, RenderOptions};
use serde_json::Value;

fn violation(from: &str, to: &str, circular: bool) -> CouplingViolation {
    CouplingViolation {
        dir_a: from.into(),
        dir_b: to.into(),
        from_module: from.into(),
        to_module: to.into(),
        line_number: 1,
        depth: 1,
        weight: 1,
        severity: 0.5,
        direction: DependencyDirection::Sibling,
        rri: 1.0,
        is_circular: circular,
        cycle_path: if circular {
            vec![from.into(), to.into()]
        } else {
            vec![]
        },
        cycle_hop_files: vec![],
        cycle_order: if circular { 2 } else { 0 },
        cycle_hop_counts: vec![],
        weakest_link: None,
        break_cost: 0,
        score_impact: 0.0,
    }
}

fn file(name: &str, path: &str) -> Module {
    Module {
        id: format!("id-{name}"),
        snapshot_id: "snap-0001".into(),
        parent_id: None,
        name: name.into(),
        path: path.into(),
        module_type: ModuleType::File,
        depth: path.matches('/').count() as i32,
    }
}

fn dep(from: &str, to: &str) -> Dependency {
    Dependency {
        from_module_id: format!("id-{from}"),
        to_module_id: format!("id-{to}"),
        line_number: 1,
    }
}

/// Pull the JSON the renderer inlined into the `<script id="noupling-data">` tag.
fn extract_data_contract(html: &str) -> Value {
    let open = "<script id=\"noupling-data\" type=\"application/json\">";
    let close = "</script>";
    let start = html
        .find(open)
        .expect("placeholder script tag must exist in template")
        + open.len();
    let end = html[start..].find(close).expect("script tag must close") + start;
    serde_json::from_str(&html[start..end]).expect("data contract must be valid JSON")
}

fn default_inputs() -> (Settings, Snapshot) {
    let settings: Settings = serde_json::from_str("{}").expect("default settings");
    let snapshot = Snapshot {
        id: "snap-0001".to_string(),
        timestamp: "2026-06-04T12:00:00".to_string(),
        root_path: "/tmp/sample-project".to_string(),
    };
    (settings, snapshot)
}

#[test]
fn template_carries_data_injection_contract() {
    // Contract between Rust and the template subproject:
    //  1. Template ships a `<script id="noupling-data" type="application/json">`
    //     element that Rust string-substitutes the Data Contract into.
    //  2. Template provides a React root the bundle mounts into.
    //  3. Bundled code reads the noupling-data block on startup.
    // The template is free to evolve its layout, components, and styling
    // without this test breaking — it only locks the injection contract.
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    // After rendering, the script tag is filled with the JSON, so we look
    // for the opening marker + JSON content.
    assert!(
        html.contains(r#"<script id="noupling-data" type="application/json">"#),
        "injection-point script tag must be present"
    );
    // React mount point
    assert!(
        html.contains(r#"id="root""#),
        "React mount point must be present"
    );
    // Bundle reads the injection point on startup
    assert!(
        html.contains("noupling-data"),
        "bundle references the injection-point id"
    );
}

#[test]
fn render_embeds_data_contract_with_format_version_1() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    assert_eq!(contract["format_version"], 1);
}

#[test]
fn nodes_emit_one_per_file_plus_package_and_container_aggregates() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();
    let modules = vec![
        file("a.rs", "src/domain/payment/a.rs"),
        file("b.rs", "src/domain/payment/b.rs"),
        file("c.rs", "src/domain/cart/c.rs"),
    ];

    let html = render(
        &modules,
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");
    let contract = extract_data_contract(&html);
    let nodes = contract["nodes"].as_array().unwrap();

    let by_id: std::collections::HashMap<_, _> = nodes
        .iter()
        .map(|n| (n["id"].as_str().unwrap().to_string(), n.clone()))
        .collect();

    // Files
    assert_eq!(by_id["src/domain/payment/a.rs"]["kind"], "file");
    assert_eq!(
        by_id["src/domain/payment/a.rs"]["parent"],
        "src/domain/payment"
    );
    // Packages (directories with direct files)
    assert_eq!(by_id["src/domain/payment"]["kind"], "package");
    assert_eq!(by_id["src/domain/cart"]["kind"], "package");
    // Container (directory with only subdirectories)
    assert_eq!(by_id["src/domain"]["kind"], "container");
    assert_eq!(by_id["src"]["kind"], "container");

    // Each node has a metrics object
    assert!(by_id["src/domain/payment/a.rs"]["metrics"].is_object());
    assert!(by_id["src/domain/payment"]["metrics"]["file_count"].is_number());
    // Container cohesion is null
    assert!(by_id["src/domain"]["metrics"]["cohesion"].is_null());
}

#[test]
fn edges_emit_one_per_dependency_with_weight_and_violates_rule_field() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();
    let modules = vec![file("a.rs", "src/a.rs"), file("b.rs", "src/b.rs")];
    let deps = vec![dep("a.rs", "b.rs"), dep("a.rs", "b.rs")];

    let html = render(
        &modules,
        &deps,
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");
    let contract = extract_data_contract(&html);
    let edges = contract["edges"].as_array().unwrap();

    assert_eq!(
        edges.len(),
        1,
        "duplicate deps coalesce into one weighted edge"
    );
    assert_eq!(edges[0]["from"], "src/a.rs");
    assert_eq!(edges[0]["to"], "src/b.rs");
    assert_eq!(edges[0]["weight"], 2);
    assert!(edges[0].get("violates_rule").is_some());
}

#[test]
fn cycles_emit_from_circular_violations_with_id_and_members() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new()
        .with_violations(vec![{
            let mut v = violation("a", "b", true);
            v.cycle_path = vec!["a".into(), "b".into(), "c".into()];
            v.cycle_order = 3;
            v.weakest_link = Some("c -> a (1 imports)".into());
            // 3 hops: a→b=14, b→c=8, c→a=1 → break_cost=1, vs_weight=14.
            v.cycle_hop_counts = vec![14, 8, 1];
            v.break_cost = 1;
            v
        }])
        .build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");
    let contract = extract_data_contract(&html);
    let cycles = contract["cycles"].as_array().unwrap();

    assert_eq!(cycles.len(), 1);
    assert!(cycles[0]["id"].as_str().unwrap().starts_with("cycle-"));
    assert_eq!(cycles[0]["size"], 3);
    let members = cycles[0]["members"].as_array().unwrap();
    assert_eq!(members.len(), 3);
    // #277: minimum_cut entries now carry weight + vs_weight so the UI
    // can render `break: c → a (1 vs 14)`.
    let cut = &cycles[0]["minimum_cut"][0];
    assert_eq!(cut["from"], "c");
    assert_eq!(cut["to"], "a");
    assert_eq!(cut["weight"], 1);
    assert_eq!(cut["vs_weight"], 14);
}

#[test]
fn violations_emit_from_audit_with_severity_and_edge_info() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new()
        .with_violations(vec![{
            let mut v = violation("src/ui/x", "src/infra/y", false);
            v.severity = 0.75;
            v
        }])
        .build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");
    let contract = extract_data_contract(&html);
    let violations = contract["violations"].as_array().unwrap();

    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["edge"]["from"], "src/ui/x");
    assert_eq!(violations[0]["edge"]["to"], "src/infra/y");
    assert_eq!(violations[0]["severity"], "high"); // > 0.5 → high per PRD example
}

#[test]
fn history_defaults_to_empty_array() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");
    let contract = extract_data_contract(&html);
    assert!(
        contract["history"].is_array(),
        "history field must be present"
    );
    assert_eq!(contract["history"].as_array().unwrap().len(), 0);
}

#[test]
fn history_block_propagates_supplied_entries_oldest_first() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();
    let options = RenderOptions {
        history: vec![
            noupling_explorer::HistoryEntry {
                snapshot_id: "s1".into(),
                taken_at: "2026-05-01T00:00:00Z".into(),
                health_score: 70.0,
            },
            noupling_explorer::HistoryEntry {
                snapshot_id: "s2".into(),
                taken_at: "2026-06-01T00:00:00Z".into(),
                health_score: 82.5,
            },
        ],
        ..Default::default()
    };

    let html =
        render(&[], &[], &audit, &settings, &snapshot, &options).expect("render must succeed");
    let contract = extract_data_contract(&html);
    let history = contract["history"].as_array().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["snapshot_id"], "s1");
    assert_eq!(history[0]["health_score"], 70.0);
    assert_eq!(history[1]["snapshot_id"], "s2");
    assert_eq!(history[1]["health_score"], 82.5);
}

#[test]
fn no_history_option_omits_history_block_content() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();
    let options = RenderOptions {
        include_history: false,
        ..Default::default()
    };

    let html =
        render(&[], &[], &audit, &settings, &snapshot, &options).expect("render must succeed");
    let contract = extract_data_contract(&html);
    assert!(contract["history"].is_array());
    assert_eq!(contract["history"].as_array().unwrap().len(), 0);
}

#[test]
fn dependency_rules_and_effective_rules_carry_source_chip() {
    let snapshot = Snapshot {
        id: "s".into(),
        timestamp: "2026-06-04T12:00:00".into(),
        root_path: "/p".into(),
    };
    let settings_json = r#"{
        "layers": [
            { "name": "ui",     "pattern": "**/ui/**" },
            { "name": "domain", "pattern": "**/domain/**" }
        ],
        "dependency_rules": [
            { "from": "**/ui/**", "to": "**/infra/**", "allow": false, "message": "ui must not reach infra" }
        ]
    }"#;
    let settings: Settings = serde_json::from_str(settings_json).unwrap();
    // Layers reach the contract through the audit result (ADR 0001), not
    // straight from settings.
    let audit = AuditResultBuilder::new()
        .with_layers(settings.layers.clone(), false)
        .build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");
    let contract = extract_data_contract(&html);

    // dependency_rules echoes the settings entries verbatim
    let rules = contract["dependency_rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["from"], "**/ui/**");
    assert_eq!(rules[0]["to"], "**/infra/**");
    assert_eq!(rules[0]["allow"], false);
    assert_eq!(rules[0]["message"], "ui must not reach infra");

    // effective_rules unions explicit dep rules + implicit layer-order rules
    // and tags each with a `source` chip per PRD §6.
    let effective = contract["effective_rules"].as_array().unwrap();
    // 1 explicit rule + 1 layer-order rule (domain may not depend on ui — ui is index 0, domain is 1)
    assert_eq!(effective.len(), 2);
    let by_source: std::collections::HashMap<_, _> = effective
        .iter()
        .map(|e| (e["source"].as_str().unwrap().to_string(), e.clone()))
        .collect();
    assert!(by_source.contains_key("dependency_rule"));
    assert!(by_source.contains_key("layer_order"));
    assert_eq!(by_source["dependency_rule"]["allow"], false);
    assert_eq!(by_source["layer_order"]["allow"], false);
    assert_eq!(by_source["layer_order"]["current_violation_count"], 0);
}

#[test]
fn layers_carry_settings_fields_plus_derived_metrics() {
    let snapshot = Snapshot {
        id: "s".into(),
        timestamp: "2026-06-04T12:00:00".into(),
        root_path: "/p".into(),
    };
    let settings_json = r#"{
        "layers": [
            { "name": "ui",     "pattern": "**/ui/**",     "allow_sibling": false },
            { "name": "domain", "pattern": "**/domain/**", "allow_sibling": true },
            { "name": "infra",  "pattern": "**/infra/**",  "allow_sibling": false }
        ]
    }"#;
    let settings: Settings = serde_json::from_str(settings_json).unwrap();
    let modules = vec![
        file("a.rs", "src/ui/a.rs"),
        file("b.rs", "src/ui/b.rs"),
        file("c.rs", "src/domain/c.rs"),
        file("d.rs", "src/infra/d.rs"),
    ];
    let deps = vec![
        dep("a.rs", "c.rs"), // ui → domain
        dep("b.rs", "d.rs"), // ui → infra
        dep("c.rs", "d.rs"), // domain → infra
    ];
    let audit = AuditResultBuilder::new()
        .with_layers(settings.layers.clone(), false)
        .build();

    let html = render(
        &modules,
        &deps,
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    let layers = contract["layers"].as_array().expect("layers must be array");
    assert_eq!(layers.len(), 3);

    // Settings-derived fields, in source order
    assert_eq!(layers[0]["name"], "ui");
    assert_eq!(layers[0]["pattern"], "**/ui/**");
    assert_eq!(layers[0]["allow_sibling"], false);
    assert_eq!(layers[0]["index"], 0);
    assert_eq!(layers[1]["name"], "domain");
    assert_eq!(layers[1]["allow_sibling"], true);
    assert_eq!(layers[1]["index"], 1);
    assert_eq!(layers[2]["index"], 2);

    // Derived: file_count per layer
    assert_eq!(layers[0]["file_count"], 2, "ui has a.rs + b.rs");
    assert_eq!(layers[1]["file_count"], 1, "domain has c.rs");
    assert_eq!(layers[2]["file_count"], 1, "infra has d.rs");

    // Derived: afferent (Ca) = edges into the layer from outside
    // ui has nothing pointing to it → 0
    // domain has 1 edge in (from ui)
    // infra has 2 edges in (from ui and from domain)
    assert_eq!(layers[0]["afferent"], 0);
    assert_eq!(layers[1]["afferent"], 1);
    assert_eq!(layers[2]["afferent"], 2);

    // Derived: efferent (Ce) = edges out from the layer to other layers
    // ui has 2 outgoing
    // domain has 1 outgoing
    // infra has 0
    assert_eq!(layers[0]["efferent"], 2);
    assert_eq!(layers[1]["efferent"], 1);
    assert_eq!(layers[2]["efferent"], 0);

    // Derived: instability I = Ce/(Ca+Ce). Layer with 0 edges → instability null.
    assert!(
        (layers[0]["instability"].as_f64().unwrap() - 1.0).abs() < 1e-6,
        "ui only outgoing → I=1"
    );
    assert!(
        (layers[1]["instability"].as_f64().unwrap() - 0.5).abs() < 1e-6,
        "domain Ca=1 Ce=1 → I=0.5"
    );
    assert!(
        (layers[2]["instability"].as_f64().unwrap() - 0.0).abs() < 1e-6,
        "infra only incoming → I=0"
    );
}

#[test]
fn health_score_and_summary_counts_come_from_audit() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new()
        .with_score(82.0)
        .with_violations(vec![violation("a", "b", false), violation("x", "y", true)])
        .with_gravity_wells(vec![GravityWell {
            module_path: "infra/db".into(),
            total_rri: 12.0,
            relationship_count: 4,
            downward_rri: 0.0,
            sibling_rri: 0.0,
            upward_rri: 12.0,
            circular_rri: 0.0,
            direction_count: 1,
        }])
        .with_red_flags(vec![RedFlag {
            flag_type: RedFlagType::FusedSibling,
            modules: vec!["a".into(), "b".into()],
            rri: 5.0,
            imports: 0,
            median_density: 0.0,
            recommendation: "merge".into(),
        }])
        .build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    assert_eq!(contract["health_score"], 82.0);
    assert_eq!(contract["summary_counts"]["violations"], 2);
    assert_eq!(
        contract["summary_counts"]["cycles"], 1,
        "circular violations counted as cycles"
    );
    assert_eq!(contract["summary_counts"]["gravity_wells"], 1);
    assert_eq!(contract["summary_counts"]["red_flags"], 1);
}

#[test]
fn score_breakdown_explains_the_health_score_math() {
    // 1 coupling + 1 cycle, each severity 0.5, score 95 → 5 points lost.
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new()
        .with_score(95.0)
        .with_total_modules(20)
        .with_violations(vec![violation("a", "b", false), violation("x", "y", true)])
        .build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    let b = &contract["score_breakdown"];
    assert_eq!(b["total_modules"], 20);
    assert_eq!(b["total_severity"], 1.0);
    assert_eq!(b["points_lost"], 5.0);
    assert_eq!(b["cycles_severity"], 0.5);
    assert_eq!(b["coupling_severity"], 0.5);
    assert_eq!(b["top_contributors"].as_array().unwrap().len(), 2);
    // Order is severity desc; both 0.5 so order is input-order.
    assert_eq!(b["top_contributors"][0]["from"], "a");
    assert_eq!(b["top_contributors"][0]["kind"], "coupling");
    assert_eq!(b["top_contributors"][1]["kind"], "cycle");
}

#[test]
fn module_enrichment_merges_into_node_metrics_llm_block() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();
    let modules = vec![file("a.rs", "src/payments/a.rs")];
    let options = RenderOptions {
        module_enrichment: vec![noupling_explorer::ModuleEnrichmentEntry {
            module_path: "src/payments".into(),
            summary: Some("Payment processing core".into()),
            responsibility: Some("Drives Stripe and Adyen flows.".into()),
            tags: vec!["domain".into()],
            generated_at: Some("2026-06-05T10:32:01Z".into()),
            model: Some("claude-opus-4-7".into()),
        }],
        ..Default::default()
    };

    let html =
        render(&modules, &[], &audit, &settings, &snapshot, &options).expect("render must succeed");
    let contract = extract_data_contract(&html);
    let nodes = contract["nodes"].as_array().unwrap();
    let payments = nodes
        .iter()
        .find(|n| n["id"] == "src/payments")
        .expect("package node must exist");
    let llm = &payments["metrics"]["llm"];
    assert_eq!(llm["summary"], "Payment processing core");
    assert_eq!(llm["tags"][0], "domain");
}

#[test]
fn codebase_counts_module_file_edge_from_inputs() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().with_total_modules(3).build();
    let modules = vec![
        file("a.rs", "src/a.rs"),
        file("b.rs", "src/b.rs"),
        file("c.rs", "src/c.rs"),
    ];
    let deps = vec![dep("a.rs", "b.rs"), dep("b.rs", "c.rs")];

    let html = render(
        &modules,
        &deps,
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    assert_eq!(
        contract["codebase"]["module_count"], 3,
        "module_count from audit.total_modules"
    );
    assert_eq!(
        contract["codebase"]["file_count"], 3,
        "file_count from modules len"
    );
    assert_eq!(
        contract["codebase"]["edge_count"], 2,
        "edge_count from dependencies len"
    );
}

#[test]
fn codebase_language_distribution_groups_files_by_extension() {
    let (settings, snapshot) = default_inputs();
    let audit = AuditResultBuilder::new().build();
    let modules = vec![
        file("a.rs", "src/a.rs"),
        file("b.rs", "src/b.rs"),
        file("c.ts", "src/c.ts"),
    ];

    let html = render(
        &modules,
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    let langs = contract["codebase"]["language_distribution"]
        .as_array()
        .expect("language_distribution must be an array");
    assert_eq!(langs.len(), 2, "two distinct extensions → two entries");
    // Sorted descending by file_count, then name asc for ties.
    assert_eq!(langs[0]["language"], "rs");
    assert_eq!(langs[0]["file_count"], 2);
    assert_eq!(langs[1]["language"], "ts");
    assert_eq!(langs[1]["file_count"], 1);
}

#[test]
fn codebase_path_comes_from_snapshot_root() {
    let (settings, _) = default_inputs();
    let snapshot = Snapshot {
        id: "s".into(),
        timestamp: "2026-06-04T12:00:00".into(),
        root_path: "/Users/me/code/acme".into(),
    };
    let audit = AuditResultBuilder::new().build();

    let html = render(
        &[],
        &[],
        &audit,
        &settings,
        &snapshot,
        &RenderOptions::default(),
    )
    .expect("render must succeed");

    let contract = extract_data_contract(&html);
    assert_eq!(contract["codebase"]["path"], "/Users/me/code/acme");
}

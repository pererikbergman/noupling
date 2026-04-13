# **Functional and Technical Requirements: noupling CLI**

## **1\. Project Overview**

**noupling** is a high-performance CLI tool designed to audit software architecture. It quantifies the "Physics of Architecture" by identifying high coupling and low cohesion through hierarchical dependency analysis.

### **1.1 Primary Goals**

* **Quantify Structural Health:** Measure coupling and cohesion metrics.  
* **Identify Gravity Failures:** Flag high-severity coupling at the root level.  
* **Enable Trend Analysis:** Track architectural drift over time using a local database.  
* **Support Multi-Language Environments:** Kotlin, C\#, Swift, TypeScript, and Rust.

## **2\. Project Architecture (Vertical Slices)**

The implementation must follow a **Vertical Slice Architecture** within the src/ directory. Each slice must be self-contained, owning its logic and data structures.

src/  
├── main.rs              \# CLI Entry point (Clap)  
├── slices/  
│   ├── scanner/         \# File discovery (Rayon) & Tree-Sitter parsing  
│   ├── storage/         \# SQLite migrations & Repository patterns  
│   ├── analyzer/        \# Bottom-up aggregation & Top-down BFS  
│   └── reporter/        \# JSON/Markdown generation  
├── core/                \# Shared domain types (Node, Dependency)  
└── utils/               \# Error handling & Logging

## **3\. Database Specification (SQLite)**

The tool uses SQLite for local persistence. The schema must be initialized automatically on the first run within .noupling/history.db.

### **3.1 Schema**

CREATE TABLE snapshots (  
    id TEXT PRIMARY KEY,        \-- UUID  
    timestamp DATETIME DEFAULT CURRENT\_TIMESTAMP,  
    root\_path TEXT NOT NULL  
);

CREATE TABLE nodes (  
    id TEXT PRIMARY KEY,  
    snapshot\_id TEXT REFERENCES snapshots(id),  
    parent\_id TEXT REFERENCES nodes(id),  
    name TEXT NOT NULL,  
    path TEXT NOT NULL,  
    node\_type TEXT CHECK(node\_type IN ('FILE', 'DIR')),  
    depth INTEGER NOT NULL  
);

CREATE TABLE dependencies (  
    from\_node\_id TEXT REFERENCES nodes(id),  
    to\_node\_id TEXT REFERENCES nodes(id),  
    line\_number INTEGER,  
    PRIMARY KEY (from\_node\_id, to\_node\_id, line\_number)  
);

## **4\. Parsing Engine (Tree-Sitter)**

The scanner slice must use Tree-Sitter grammars. It must map language-specific imports to fully qualified file paths within the project.

### **4.1 Grammar Mapping**

* **Kotlin:** Query (import\_list (import\_header (identifier) @import))  
* **TypeScript:** Query (import\_declaration (string\_literal) @import)  
* **Swift:** Query (import\_declaration (import\_kind)? (identifier) @import)  
* **C\#:** Query (using\_directive (qualified\_name) @import)  
* **Rust:** Query (use\_declaration (use\_list (use\_item (path\_expression) @import))) or (use\_declaration (path\_expression) @import)

## **5\. Mathematical Metrics & Analysis**

### **5.1 Bottom-Up Aggregation (D\_acc)**

For every Directory Node (N), D\_acc(N) is the union of all dependencies of its sub-tree.

**Constraint:** If an accumulated dependency points to a file inside the same directory, it is considered internal and excluded from the D\_acc set for sibling analysis.

### **5.2 Top-Down BFS Audit**

1. Start at depth \= 0\.  
2. For every sibling pair (A, B) at the current depth:  
   * If D\_acc(A) references a file in D\_acc(B), it is a **Coupling Violation**.  
3. **Severity Calculation:** Severity (S) \= 1 / (Depth \+ 1).

### **5.3 Architectural Health Score**

Calculated on a scale of 0-100:

Score \= 100 \* (1 \- (Sum of all Severity values / Total Modules))

## **6\. CLI Command Interface**

Built using clap:

* noupling scan \<PATH\>:  
  * Perform parallel scan (Rayon).  
  * Parse AST (Tree-Sitter).  
  * Populate SQLite.  
* noupling audit \[--snapshot ID\]:  
  * Execute BFS logic on the latest or specified snapshot.  
  * Return violations sorted by Severity.  
* noupling report \--format \<json|md\>:  
  * Output findings. JSON must include critical\_violations and score.

## **7\. Non-Functional Requirements**

* **Concurrency:** File scanning and AST parsing must use a thread pool.  
* **Memory:** Set union operations for D\_acc must use efficient hashing (e.g., fxhash or fnv).  
* **Environment:** The tool must be a single static binary.  
* **Formatting:** Never use the long dash character (double hyphen) in CLI text output.

## **8\. Development and Testing**

### **8.1 Local Build**

The project is built using standard Rust tooling:

* Build: cargo build \--release  
* Output Binary Location: ./target/release/noupling

### **8.2 Testing Strategy**

* **Unit Tests:** Each slice (analyzer, scanner) must have unit tests.  
* **Integration Tests:** Run the tool against a mock directory structure in tests/fixtures.  
* Command: cargo test

## **9\. Local Deployment and Execution**

### **9.1 Direct Execution**

To run the tool during development without manual binary management:

cargo run \-- scan /path/to/target/project

### **9.2 Installation**

To install the tool globally on your machine:

cargo install \--path .

## **10\. Failure Modes**

* **Circular Dependencies:** Detected during BFS. Must be flagged as a "Structural Loop" (Severity: 1.0).  
* **Missing Grammars:** If a file type is found but the grammar is not bundled, log a warning and skip the file.

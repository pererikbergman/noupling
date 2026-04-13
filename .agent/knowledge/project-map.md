---
description: High-level overview of project purpose, domain, and architectural boundaries
type: Knowledge
---

# Project Map

## Purpose

A modular Rust workspace project using Cargo Workspaces, designed with agent-ready decoupling principles.

## Domain

General-purpose service architecture with a binary API application backed by shared business logic and common data types.

## Architectural Boundaries

| Crate | Type | Responsibility |
| :--- | :--- | :--- |
| `api_app` | Binary | Primary HTTP service entry point, routing, and request handling |
| `core_logic` | Library | Internal business logic, domain rules, and error definitions |
| `shared_types` | Library | Common DTOs, models, and serialization structures |

## Key Directories

- `crates/`: All internal Rust crates (workspace members)
- `deploy/`: Dockerfile and docker-compose for containerization
- `docs/`: Design documents, API specs, and product specifications
- `scripts/`: Development and automation scripts
- `tests/`: Integration and common test utilities
- `.agent/`: Agent rules, skills, workflows, and knowledge base

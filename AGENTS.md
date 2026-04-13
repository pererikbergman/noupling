# Agent Instructions

- Workspace: Cargo Workspace architecture.
- Toolchain: Managed via `rust-toolchain.toml`.
- Crate Strategy:
    - `api_app`: Primary binary/service.
    - `core_logic`: Internal business logic (Library).
    - `shared_types`: Common DTOs and models (Library).
- Formatting: Always run `cargo fmt` after changes.
- Linting: Use `cargo clippy` for validation.

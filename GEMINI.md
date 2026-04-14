# MotoManager API — Gemini CLI Instructions

## Core Principles
You are Gemini CLI, the expert backend architect for MotoManager. Follow these project-specific mandates:

### Architecture
- **Library/Binary Split**: Core logic resides in `src/lib.rs`. `src/main.rs` is only for server entry. Integration tests in `tests/` must use the library.
- **CamelCase Consistency**: All database tables, columns, and JSON API keys MUST be camelCase.
- **Axum Handlers**: Return `AppResult<T>` and use the proper extractors (`AuthUser`, `AdminUser`, `State(pool)`, etc.).
- **Latest Stack**: Use Axum 0.8 and SQLx 0.8. Note the `{id}` syntax for path parameters in Axum 0.8.

### Testing & Quality Standards
- **Validation is Mandatory**: Every feature or bug fix MUST include corresponding tests.
- **Linting is Mandatory**: Code MUST be clean and pass Clippy checks (with no warnings) and formatting checks after EVERY change.
- **Verification is Mandatory**: Run `cargo clippy` and `cargo test` after each modification to ensure regressions or lint issues are not introduced.
- **Test Types**:
    - **Unit Tests**: Place in the same file as the logic being tested (use `#[cfg(test)] mod tests`).
    - **Integration Tests**: Place in the `tests/` directory. Use `setup_test_app` (from existing tests) for a clean in-memory environment.
- **Commands**:
    ```sh
    cargo test                # Run all tests
    cargo test --test <name>  # Run specific integration test
    cargo clippy --all-targets --all-features -- -D warnings # Run linter (must pass with 0 warnings)
    cargo fmt --all           # Format code (must be run before committing)
    ```

### Workflow
1.  **Research**: Use `grep_search` and `read_file` to understand the current handler/model logic.
2.  **Reproduction**: For bugs, write a failing integration test in `tests/` before applying the fix.
3.  **Implementation**: Follow existing patterns in `src/handlers/`. Ensure all JSON mappings in `row_to_value` are correct.
4.  **Verification**: After EVERY modification, run the full suite with `cargo test` and `cargo clippy`. Ensure zero warnings and zero errors. Always run `cargo fmt` before concluding a task.

## Common Tasks

### Adding a New Entity
1.  Create migration in `migrations/`.
2.  Add model to `src/models.rs`.
3.  Create handler in `src/handlers/`.
4.  Register routes in `src/lib.rs` (using `{id}` syntax).
5.  Add integration test in `tests/`.

### Modifying the Schema
1.  Add a new migration file.
2.  Update the models and handlers.
3.  **Crucial**: Update the `Dockerfile` schema preparation step (the `RUN touch db.sqlite && ...` block) to include the new migration file. This is required for `sqlx` macro validation during the Docker build.
4.  **Crucial**: The dev database `db.sqlite` might need to be recreated if schema changes are destructive (no auto-migration tool currently beyond what SQLx provides).

### File Uploads
- Use `save_image` or `save_document_file` helpers in handlers.
- Previews are automatically generated for images and PDFs.

## Performance & Scaling
- Ensure all foreign keys (`motorcycleId`, `userId`, `locationId`) have indexes in migrations for query performance.
- Use `recalculate_fuel_consumption` logic for fuel-related entries.

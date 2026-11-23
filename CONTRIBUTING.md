# Contributing to OpenScale

Thank you for your interest in contributing to **OpenScale**!

## License

By contributing to OpenScale, you agree that your contributions will be licensed under the **Apache License 2.0** (`LICENSE`).

## Development Workflow

1. Fork the repository and create a feature branch from `main`.
2. Ensure the Rust workspace compiles cleanly and passes all unit tests and lints:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
3. Ensure TypeScript packages typecheck and build cleanly:
   ```bash
   pnpm typecheck
   pnpm build
   ```

## Testing Against Physical Scales (Opt-in)

Unit tests must remain 100% deterministic and run without physical hardware.

Hardware integration tests are opt-in and controlled via environment variables:
- `OPENSCALE_TEST_XS_IP` — IP address of an Avery Berkel XS / M-Series scale
- `OPENSCALE_TEST_XTI_IP` — IP address of an Avery Berkel XTi / XTRA scale
- `OPENSCALE_TEST_PLU` — Reserved test PLU number for non-destructive read/write/restore checks

Never write destructive tests that modify or clear production PLUs or machine parameters without restoring their original bytes.

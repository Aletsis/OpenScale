# OpenScale

Open-source (**Apache-2.0**) cross-platform suite for managing, integrating, and diagnosing **Avery Berkel** retail scales (**XS / M-Series** and **XTi / XTRA**), architected to scale from a local desktop app to multi-store deployments (Cloud + Local Agent) and future scale brands.

## Documentation

- [Implementation Plan & Architecture](./docs/plan-implementacion.md)
- [XS / M-Series Protocol Specification](./docs/protocolo-xs-mseries.md) (TCP `3001`, `XsServicePort` TCP `28822`, UDP `23452`)
- [XTi / XTRA Protocol Specification](./docs/protocolo-xti-xtra.md) (TCP `3002`, UDP `23453`)

## Monorepo Layout

```text
OpenScale/
├── crates/
│   ├── openscale-domain/      # Pure domain models, PluKey, MoneyCents, RBAC & errors
│   ├── openscale-driver/      # ScaleDriver trait, DriverCapabilities & events
│   └── openscale-storage/     # SQLx SQLite (WAL) migrations & repositories
├── packages/
│   ├── i18n/                  # English (en) & Spanish (es) translations (i18next)
│   ├── client/                # TypeScript domain types & OpenScaleTransport interface
│   └── ui/                    # Shared React + Tailwind + TanStack UI kit
├── docs/                      # Architecture plan and validated protocol docs
└── scripts/                   # Validated PowerShell protocol prototypes
```

## Quick Start

### Prerequisites

- **Rust** (stable toolchain via `rustup`)
- **Node.js** 22+ LTS and **pnpm** 10+

### Common Commands

```bash
# Rust checks & tests
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# TypeScript checks & builds
pnpm install
pnpm typecheck
pnpm build
```

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).

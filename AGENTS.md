# Agent guide

Reply in Chinese unless requested otherwise. Use English Conventional Commits, commit bodies, PR titles, and PR descriptions.

## Read for the task

| Task | Read |
| --- | --- |
| Any change | [Contributing](CONTRIBUTING.md): implementation, checks, and delivery |
| Product behavior | [Product contract](docs/PRODUCT.md) |
| Desktop, Rust, data, or APIs | [Architecture](ARCHITECTURE.md) |
| Website, content, SEO/GEO | [Website](website/README.md) and [SEO/GEO](website/geo/README.md) |
| Version or release | [Release workflow](docs/RELEASING.md), [installation/update mechanics](docs/INSTALL_AND_UPDATES.md), and [changelog](CHANGELOG.md) |
| Documentation or internal records | [Documentation policy](docs/README.md) |

## Working rules

- Inspect Git status first; preserve unrelated work. Trace the source of truth, owner, data flow, and consumers before changing behavior. Reuse existing abstractions and make the smallest correct change.
- Keep Rust authoritative for business data and authorization; React owns interaction state. Generate DTOs with `pnpm types` after changing `model.rs`; do not hand-edit generated contracts or website outputs.
- Follow existing style and dependency locks. Avoid unrelated refactors, upgrades, broad fallbacks, and silent errors. Add brief Chinese comments for non-obvious logic.
- Use only approved temporary fixtures. Never test against real work files, production databases, or backups, or clear existing directories. Keep credentials and raw/private evidence out of commits.
- Run the relevant checks in CONTRIBUTING.md. Report passed, failed, blocked, and unrun checks accurately; native/platform evidence cannot be replaced by mocks or builds. Do not weaken checks or safety requirements to pass.
- Update the owning document when behavior, architecture, support, or commands change; record user-visible changes under `Unreleased`. Public maintenance must work without `.internal/` or previous conversations.
- Complete authorized work and verification without repeated confirmations. Commit/push when requested; publication, deployments, and signing-key changes need explicit task authorization. Do not rewrite history as routine cleanup.
- Close out with the change, validation, remaining limits, and Git/publication state. On interruption, leave the exact build, results, blockers, and next action in an appropriate sanitized handoff; keep raw evidence private.

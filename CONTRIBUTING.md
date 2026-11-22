# Contributing to WorkTwill

Bug reports, usability feedback, and focused fixes are welcome. For a new feature or a change to product behavior, discuss its scope in a GitHub Issue first. A maintainer-assigned task already provides that scope; routine fixes do not need a separate proposal.

## Start here

Read [AGENTS.md](AGENTS.md) for development rules, [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and invariants, and the [documentation index](docs/README.md) for task-specific guides. Public documentation, source, tests, and the current issue must be sufficient to work on the project. Internal planning and historical acceptance reports are not contribution prerequisites.

Use Node.js 22 LTS, the pnpm version in `package.json`, the pinned Rust toolchain, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). Install dependencies in this workspace only:

```sh
pnpm install --frozen-lockfile
pnpm desktop
```

`pnpm dev` previews the UI in a browser; it cannot validate native permissions or file operations.

## Make and verify a change

Trace the existing data flow and consumers before editing. Keep changes focused and preserve Rust ownership of authorization, identity, relationships, and persisted data. After changing Rust DTOs in `model.rs`, run `pnpm types` and include the generated contract changes.

Use only an explicitly approved temporary directory for generated test fixtures. Tests create their own subdirectories; never use real documents or a production database.

```sh
pnpm check --fixtures /absolute/path/to/approved-fixture-root
```

This checks generated DTOs, TypeScript and Vite, UI tests, Rust formatting, Rust workspace tests, and Clippy. Rust, native-shell, DTO, and cross-layer behavior changes require this full check. Website changes use `pnpm website:check`; changes to its HTTP audit also use `node --test website/scripts/audit-geo.test.mjs`. Documentation-only changes require checking links, commands, and public/private boundaries. Run explicit benchmarks only when relevant; see [architecture](ARCHITECTURE.md#verification-and-performance).

Add a regression test when it can detect the reported behavior failure. Do not remove failing checks or weaken permission, recovery, or performance requirements to make a change pass. Native interactions, editor saves, installation, and updates require observations on the relevant platform and build. Report unavailable checks as not run, with the reason.

## Commits and pull requests

Use English for commit subjects and bodies, PR titles, and PR descriptions. Use Conventional Commits for new commits, for example:

```text
fix(search): reject stale pagination results
docs: clarify platform support boundaries
ci: check Windows installer artifacts
```

A PR should explain the problem and resulting behavior, relevant validation, and any remaining limitations. Keep unrelated edits out of the diff. Changes to authorization, identity, migrations, recovery, or verification requirements need an explicit explanation of their effect. Existing commit history does not need to be rewritten.

## Documentation and private data

Update public documentation when behavior, commands, ownership, or supported scope changes. Publish concise, sanitized design decisions when they help future maintenance. Keep private product plans, raw acceptance evidence, screenshots, personal paths, databases, backups, credentials, and generated artifacts out of commits.

Maintainers may keep local internal material under the ignored `.internal/` directory. It is optional and requires a separate private backup. Ignoring or deleting a file does not remove it from Git history; review history separately before making a repository public.

## Distribution and licensing

Local builds and CI artifacts are separate from publication. Follow [installation and updates](docs/INSTALL_AND_UPDATES.md) for release preparation. Publishing packages, deploying the website, changing update manifests, or rotating signing keys requires the maintainer's explicit authorization for that task. Never attach signing keys to issues, logs, or artifacts.

Contributions are provided under the project's [MIT License](LICENSE), unless explicitly agreed otherwise. Third-party components retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

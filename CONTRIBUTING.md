# Contributing to WorkTwill

Bug reports, usability feedback, and focused fixes are welcome. For a new feature or a change to product behavior, discuss its scope in a GitHub Issue first. A maintainer-assigned task already provides that scope; routine fixes do not need a separate proposal.

## Start here

Read [AGENTS.md](AGENTS.md) for the short agent entry point and task routing, the [product contract](docs/PRODUCT.md) for expected behavior, and [ARCHITECTURE.md](ARCHITECTURE.md) for technical ownership. The [documentation index](docs/README.md) lists other task-specific guides. Public documentation, source, tests, and the current issue must be sufficient to work on the project. Internal planning and historical acceptance reports are not contribution prerequisites.

Use Node.js 22 LTS, the pnpm version in `package.json`, the pinned Rust toolchain, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). Install dependencies in this workspace only:

```sh
pnpm install --frozen-lockfile
pnpm desktop
```

`pnpm dev` previews the UI in a browser; it cannot validate native permissions or file operations.

## Make and verify a change

Trace the existing data flow and consumers before editing. Keep changes focused and preserve Rust ownership of authorization, identity, relationships, and persisted data. After changing Rust DTOs in `model.rs`, run `pnpm types` and include the generated contract changes.

Follow existing code style and implementations. Keep independent configuration sections as page-private controlled components when useful; retain page ownership of requests and submission rather than adding a hook/store just for extraction. Reuse validation appropriate to the input's meaning and brief existing feedback patterns. Maintain both UI languages and supplied wording; avoid extra error state, explanatory copy, or interactions unless the task or existing form requires them.

Add brief Chinese comments for non-obvious new logic. Avoid unrelated refactors, formatting, renames, dependency upgrades, broad fallbacks, and silent error handling. Install dependencies only in this workspace; preserve toolchain/dependency locks and update the relevant lockfile when an intentional change requires it.

Use only an explicitly approved temporary directory for generated test fixtures. Tests create their own subdirectories; never use real documents or a production database.

```sh
pnpm check --fixtures /absolute/path/to/approved-fixture-root
```

This checks generated DTOs, TypeScript and Vite, UI tests, Rust formatting, Rust workspace tests, and Clippy. Rust, native-shell, DTO, and cross-layer behavior changes require this full check. Website changes use `pnpm website:check`; changes to its HTTP audit also use `node --test website/scripts/audit-geo.test.mjs`. Release tooling changes also run `pnpm test:release` and `cargo test --locked -p worktwill-desktop --features release-tools --bin verify-release`; Rust release verifier changes require the full project check. Documentation-only changes require checking links, commands, and public/private boundaries. Run explicit benchmarks only when relevant; see [architecture](ARCHITECTURE.md#verification-and-performance).

Windows continuity fixtures explicitly assign Object IDs to disposable files to cover anchor-preserved and anchor-absent replacements. Run these tests in a disposable administrator session whose token has `SeRestorePrivilege` (as on the Windows CI runner). Only the test process enables this existing privilege and opens its synthetic files with backup semantics; the desktop application does not request it. Missing fixture privileges fail explicitly rather than skipping continuity assertions.

Add a regression test when it can detect the reported behavior failure. Do not remove failing checks or weaken permission, recovery, or performance requirements to make a change pass. Native interactions, editor saves, installation, and updates require observations on the relevant platform and build. Report unavailable checks as not run, with the reason.

## Automated checks and dependency maintenance

The [Project checks workflow](.github/workflows/check.yml) runs `pnpm check` on pull requests and pushes to `main`, using the pinned toolchain on macOS and Windows without release secrets. It also runs the isolated website HTTP audit tests. These checks do not package, publish, or establish native installation/editor/update acceptance. Full website checks still require the exact registered artifacts described in [website engineering](website/README.md).

Dependabot checks npm, Cargo, and GitHub Actions weekly with bounded open PRs. Minor/patch updates are grouped; updates are reviewed and checked rather than automatically merged. Toolchain and application versions still follow their owning configuration and release process.

After this workflow has run successfully on GitHub, maintainers should require its macOS and Windows checks on `main` and block force pushes/deletion. Do not require a status that has not been registered by a real run, or require a second reviewer in a single-maintainer project. Local workflow validation is not proof that remote protection is enabled.

## Commits and pull requests

Use English for commit subjects and bodies, PR titles, and PR descriptions. Use Conventional Commits for new commits, for example:

```text
fix(search): reject stale pagination results
docs: clarify platform support boundaries
ci: check Windows installer artifacts
```

A PR should explain the problem and resulting behavior, relevant validation, and any remaining limitations. Keep unrelated edits out of the diff. Changes to authorization, identity, migrations, recovery, or verification requirements need an explicit explanation of their effect. Existing commit history does not need to be rewritten.

Commit and push only when requested, staging the intended files after review. Report the exact validation and Git/publication state when handing off. On interruption, record the source/build, completed work, failures or unrun checks, blockers, and next action; keep raw/private evidence internal rather than in public PR text.

## Documentation and private data

Update the owning public document when behavior, commands, ownership, or supported scope changes. Record user-visible changes in [CHANGELOG.md](CHANGELOG.md); follow the [release workflow](docs/RELEASING.md) for versioned delivery. Publish concise, sanitized design decisions when they help future maintenance. Keep private product plans, raw acceptance evidence, screenshots, personal paths, databases, backups, credentials, and generated artifacts out of commits.

Maintainers may keep local internal material under the ignored `.internal/` directory. It is optional and requires a separate private backup. Ignoring or deleting a file does not remove it from Git history; review history separately before making a repository public.

## Security reports

Report vulnerabilities through the private channel in [SECURITY.md](SECURITY.md), not public issues. Ordinary bugs and feature proposals use the issue forms. Do not include private documents, databases, paths, or credentials in either issues or pull requests.

## Distribution and licensing

Local builds and CI artifacts are separate from publication. Follow [installation and updates](docs/INSTALL_AND_UPDATES.md) for release preparation. Publishing packages, deploying the website, changing update manifests, or rotating signing keys requires the maintainer's explicit authorization for that task. Never attach signing keys to issues, logs, or artifacts.

Contributions are provided under the project's [MIT License](LICENSE), unless explicitly agreed otherwise. Third-party components retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

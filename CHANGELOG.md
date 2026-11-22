# Changelog

User-facing changes are collected under `Unreleased` and moved to dated version sections when published. Release entries identify the available platforms and relevant upgrade limitations. This file starts with the current maintenance workflow; it does not reconstruct historical releases from internal candidates.

## Unreleased

### Fixed

- Fail snapshot copying promptly on database lock contention instead of waiting in SQLite's busy handler while holding the application data gate; preserve the existing explicit-retry and data-preservation behavior.
- Handle both LF and CRLF source files in isolated website audit tests.

### Maintenance

- Add secret-free macOS and Windows checks for pull requests and main-branch pushes, including isolated website HTTP audit tests.
- Add bilingual bug/feature forms, a pull request checklist, private security reporting guidance, and weekly grouped dependency update configuration.

### Documentation

- Organize agent guidance, product behavior, architecture, website SEO/GEO maintenance, and version iteration into focused maintenance documents.
- Define candidate promotion, artifact provenance and archiving, upload/publication verification, website-only deployment, and release withdrawal workflows, with current automation gaps stated explicitly.

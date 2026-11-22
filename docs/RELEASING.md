# Version iteration and release workflow

Use this document to carry a scoped fix or feature through maintenance and release. Build commands, signing, installation, CI, and update behavior remain in [INSTALL_AND_UPDATES.md](INSTALL_AND_UPDATES.md); do not maintain a second packaging procedure here.

## 1. Scope the iteration

Start from the current issue or maintainer task: define the user-visible problem, expected result, affected platforms, and completion evidence. Review the [product contract](PRODUCT.md) and affected [architecture](../ARCHITECTURE.md) boundaries. Use the existing issue/task for progress; do not create a parallel task system or resume a dormant internal handoff.

Implement a focused change with necessary regressions. Update its owning documentation and add a concise user-facing note under `Unreleased` in [CHANGELOG.md](../CHANGELOG.md) when behavior changes. Include upgrade requirements or known limitations when they affect users; keep raw acceptance logs private. Documentation-only or website-only changes do not require a desktop version bump.

## 2. Choose the version and assess compatibility

Use semantic versions for the desktop application: patch for backward-compatible fixes, minor for backward-compatible features, major for incompatible product or integration contracts. Discuss compatibility and recovery before an incompatible change. A schema migration alone does not determine the application major version; its upgrade/downgrade effects still need review and tests.

Prepare the version once the intended release scope is clear. Keep these current application/crate versions aligned:

- `package.json` → `version`;
- `src-tauri/tauri.conf.json` → `version`;
- `src-tauri/Cargo.toml` → package version;
- `crates/worktwill-core/Cargo.toml` → package version;
- corresponding local-package entries in `Cargo.lock`, refreshed by Cargo without unrelated dependency upgrades.

The release script currently checks the first two values; the four-way check is a maintainer responsibility, not an existing automated guarantee. Application identity/data paths and the existing update-signing public key must remain stable across ordinary upgrades. Use a higher version for a new public update; rebuilding the same version will not update installed clients.

| Value or record | Authority | Meaning |
| --- | --- | --- |
| Application version | Files listed above | Version being built; not evidence it is published |
| Database schema | `crates/worktwill-core/src/migrations.rs` | Storage compatibility; version independently of the application |
| Per-platform release | `website/release.json`, written by `release:record` | Exact installer/update artifacts, version, size, hash, signature, and stage; platforms may advance separately |
| User-facing history | `CHANGELOG.md` | Reviewed changes and limitations, not build or acceptance logs |

Do not set a platform's record to a new version until that platform has the corresponding verified artifacts. A local registry entry or its timestamp does not establish online availability.

## 3. Validate the candidate

Run the checks required by [CONTRIBUTING.md](../CONTRIBUTING.md) and native checks relevant to this change on each affected platform. For migrations, test existing-data upgrades, failure preservation, and restore permissions. For updater changes, test pending saves, version/signature handling, upgrade, restart, and retained data.

Bind results to source and artifact hashes. Mark each check passed, failed, blocked, or not run; distinguish local fixtures, CI packaging, native behavior, and online download/update. An unavailable environment is a gap to report, not permission to claim support or rerun unrelated checks indefinitely.

## 4. Package and publish

With release authorization, follow the existing [packaging and registration procedure](INSTALL_AND_UPDATES.md). Keep signed, immutable artifacts and their exact platform records together. Upload and verify all referenced artifacts before publishing the website/update manifests. Never replace an existing artifact's bytes under the same URL or rotate keys to fix packaging.

Move only the shipped entries from `Unreleased` into a dated version section when publishing, with explicit platform scope and remaining limitations. If platforms ship separately, state which are available; do not declare the other platform released. Keep any GitHub Release notes consistent with this entry and the registry. Do not invent historical release entries from candidate versions or test reports.

Source commit/push, tags/GitHub Releases, artifact uploads, and website/update publication are distinct actions. Perform those requested by the task and report which actually completed. Use `v<version>` for an authorized release tag and bind it to the candidate source commit; do not move published tags. Authorization to fix or push source alone does not authorize distribution.

## 5. Synchronize the website and follow up

When shipped behavior, support, privacy, or install steps change, update both website languages and the relevant product, guide, download, or privacy pages from their source templates. Review [SEO/GEO maintenance](../website/geo/README.md): metadata, routes/redirects, structured facts, dates, sitemap, and public support claims must agree. Do not announce an unshipped feature through SEO content.

Verify the actual public pages, installer bytes, and update manifests after an authorized deployment. Track reported regressions through existing issues. Compare search/index/citation observations in matching windows; do not confuse a successful HTTP audit with discovery or adoption.

If a published version fails, first prevent additional distribution using the documented publication path and explicit task authorization. Preserve the faulty artifacts and failure evidence privately. Prefer a verified forward patch; reinstalling an older binary does not roll back a migrated database. Assess the matching data snapshot and schema before proposing rollback, and communicate user impact. Do not silently downgrade, overwrite artifacts, or describe an untested recovery as working.

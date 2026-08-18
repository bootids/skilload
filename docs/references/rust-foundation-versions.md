# Rust Foundation Versions

Scope: the initial Rust workspace, configuration slice, test dependencies, and CI action pins selected for skilload on 2026-08-18.

Last updated: 2026-08-18.

## Why It Matters

The repository baseline deliberately deferred exact Rust and crate versions to the first implementation delivery. Pinning the toolchain and committing `Cargo.lock` makes local and CI builds repeatable, while recording the selected direct dependencies prevents later Plans from rediscovering why the foundation uses them.

## Key Conclusions

* Rust 1.97.1 is the current stable point release. The workspace should use Rust edition 2024, set `rust-version = "1.97.1"`, and pin the same version in both `mise.toml` and `rust-toolchain.toml`.
* Rust has provided `std::fs::File::lock`, shared locking, nonblocking locking, and unlocking since 1.89. The configuration slice therefore does not need a third-party file-lock crate.
* The crates.io registry reported these newest non-yanked stable direct versions: `clap 4.6.6`, `serde 1.0.229`, `serde_json 1.0.151`, `toml 1.1.4+spec-1.1.0`, `thiserror 2.0.20`, `base64 0.23.1`, `tempfile 3.27.0`, `assert_cmd 2.2.2`, and `predicates 3.1.4`. Their declared minimum Rust versions are all at or below 1.85.
* Cargo ignores SemVer build metadata in dependency requirements. Declare the TOML dependency as version `1.1.4`; the committed lockfile records the registry package's full `1.1.4+spec-1.1.0` version.
* Use ordinary compatible direct requirements with all three numeric components and commit `Cargo.lock`. The lockfile, rather than artificially narrow upper bounds, is the exact dependency snapshot for the binary workspace.
* The initial CI workflow should pin `actions/checkout` major tag `v5` to commit `fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09` and `jdx/mise-action` major tag `v3` to commit `5228313ee0372e111a38da051671ca30fc5a96db`. Those immutable commits were the tag targets returned by GitHub on 2026-08-18.
* Node.js, npm, and pnpm are not needed by this slice. Any later repository tooling that adds them must pin them through the root `mise.toml` rather than introducing a parallel toolchain.
* CI run [#32144984316](https://github.com/bootids/skilload/actions/runs/32144984316) passed on Ubuntu 24.04 and macOS 15 with the pinned `jdx/mise-action` SHA. GitHub emitted a non-failing warning that this action targets deprecated Node.js 20 and was forced onto Node.js 24. Keep the immutable pin for this delivery; reevaluate and revalidate an action upgrade deliberately rather than changing it incidentally.

## Selected Dependency Roles

`clap` owns the real, currently implemented command schema; automatic help subcommands and color can be disabled so no extra command or uncontrolled terminal escape enters the development surface. `serde`, `serde_json`, and `toml` own typed serialization instead of string manipulation. `thiserror` supports typed internal errors, `base64` implements the required padded standard-alphabet native-path representation, and `tempfile` provides exclusive same-directory staging for atomic configuration replacement.

`assert_cmd` and `predicates` are test-only CLI helpers. The standard library supplies filesystem locking and terminal detection, so no locking or TTY crate is selected.

## Cautions

Registry latest versions and moving action tags will change. Do not silently update this reference or the lockfile as incidental work; dependency upgrades need an intentional diff, the same validation matrix, and an updated date. Full action commit pins are immutable, but their upstream major tags may later point elsewhere.

The Rust toolchain pin is a development baseline, not yet the four-target 0.1 release matrix. Release packaging, SQLite/FTS5, HTTP, GitHub, Agent, and cryptographic dependencies remain outside this configuration delivery.

## Sources

* [Rust 1.97.1 release announcement](https://blog.rust-lang.org/2026/07/16/Rust-1.97.1/)
* [Rust `std::fs::File` locking documentation](https://doc.rust-lang.org/std/fs/struct.File.html)
* [Cargo dependency-version guidance](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
* [Cargo lockfile rationale](https://doc.rust-lang.org/cargo/faq.html#why-have-cargolock-in-version-control)
* [crates.io API: clap](https://crates.io/api/v1/crates/clap)
* [crates.io API: serde](https://crates.io/api/v1/crates/serde)
* [crates.io API: serde_json](https://crates.io/api/v1/crates/serde_json)
* [crates.io API: toml](https://crates.io/api/v1/crates/toml)
* [crates.io API: thiserror](https://crates.io/api/v1/crates/thiserror)
* [crates.io API: base64](https://crates.io/api/v1/crates/base64)
* [crates.io API: tempfile](https://crates.io/api/v1/crates/tempfile)
* [crates.io API: assert_cmd](https://crates.io/api/v1/crates/assert_cmd)
* [crates.io API: predicates](https://crates.io/api/v1/crates/predicates)
* [GitHub tag reference: actions/checkout v5](https://api.github.com/repos/actions/checkout/git/ref/tags/v5)
* [GitHub tag reference: jdx/mise-action v3](https://api.github.com/repos/jdx/mise-action/git/ref/tags/v3)

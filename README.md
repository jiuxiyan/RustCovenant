# RustCovenant: artifact

This artifact accompanies a protected acceptance-protocol study. It contains all seven fixed tasks, fourteen existing candidates, two generated-test methods, the original strict policy, and a separately reported post-hoc response-format sensitivity analysis. See [REPRODUCIBILITY.md](REPRODUCIBILITY.md) for claim-to-file coverage, study design, and limitations.

## Quick offline audit

Requirements: Python 3.11 or later; no third-party Python packages, compiler, network, model credentials, or external service are needed for the audit.

```sh
python3 -B run_checks.py
```

Expected output is JSON with `"status": "PASS"`. The command checks `SHA256SUMS`, source/material hashes, original responses, generated programs, and their bindings to 46 execution records. It recomputes 194 process classifications, 56 generated-method decisions, and 28 baseline decisions, then runs all 1,368 protocol assertions in a temporary directory.

To run only comparison recomputation, use `python3 -B audit.py`. Keep new outputs outside this directory. Git metadata and Python bytecode caches are excluded from the package inventory; all archived files remain hash-checked.

## What the numbers mean

Each row has 14 candidates. A is all accepted; S is accepted and reference-supported; V is accepted with a confirmed reference violation; U is accepted with an uncertain oracle; R is rejected; Ab is abstained; G is admitted generated suites out of seven. A = S + V + U and A + R + Ab = 14. G does not apply to B1/B2.

| Policy | Method | A | S | V | U | R | Ab | G |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| strict | B1-witness | 13 | 10 | 2 | 1 | 1 | 0 | — |
| strict | B2-ordinary | 13 | 10 | 2 | 1 | 1 | 0 | — |
| strict | B3-generic | 6 | 6 | 0 | 0 | 2 | 6 | 4 |
| strict | RustCovenant-Verify | 7 | 6 | 0 | 1 | 1 | 6 | 4 |
| normalized | B3-generic | 6 | 6 | 0 | 0 | 2 | 6 | 4 |
| normalized | RustCovenant-Verify | 8 | 7 | 0 | 1 | 2 | 4 | 5 |

SUPPORTED means support within the finite reference execution scope, not semantic correctness or soundness. T06-B1 remains ORACLE_UNCERTAIN and is never counted as a correct repair. The normalized result is a post-hoc sensitivity analysis; generic generation still distinguishes the StackVec candidates where obligation-guided generation abstains.

## Tasks

| ID | Crate | Original | Fixed |
| --- | --- | --- | --- |
| T01 | toodee | 0.2.1 | 0.3.0 |
| T02 | slab | 0.4.10 | 0.4.11 |
| T03 | stable-vec | 0.4.2 | 0.4.3 |
| T04 | caja | 0.2.1 | 0.3.0 |
| T05 | stack_collections | 0.3.2 | 0.3.3 |
| T06 | aligned_box | 0.3.0 | 0.3.1 |
| T07 | rtrb | 0.3.4 | 0.3.5 |

## Replay an archived program

Replay requires Linux x86_64, Python 3.11+, the `cryptography` package, bubblewrap with user namespaces enabled, and Rust nightly-2025-03-01 with Miri and rust-src plus a prepared Miri sysroot. Compilation and execution occur without network access in a narrow bubblewrap sandbox. The sandbox is a local research boundary, not a hardened multi-tenant service. Use a disposable authorized environment for untrusted candidate code.

Install local dependencies separately from the offline audit:

```sh
python3 -m pip install cryptography
rustup toolchain install nightly-2025-03-01 --component miri --component rust-src
cargo +nightly-2025-03-01 miri setup
```

Set `RUSTCOV_TOOLCHAIN` to the toolchain root containing `bin/miri`, `RUSTCOV_MIRI_SYSROOT` to the directory created by Miri setup, and `RUSTCOV_BWRAP` to the installed bubblewrap executable. `rustc +nightly-2025-03-01 --print sysroot` identifies the toolchain root. Miri setup reports its sysroot location. For example, in a shell after setting those three environment variables:

```sh
python3 -B replay.py --policy normalized --task T03 --method RustCovenant-Verify --variant fixed --out ../replay-T03-fixed
python3 -B replay.py --policy normalized --task T03 --method RustCovenant-Verify --variant B1 --out ../replay-T03-B1
python3 -B replay.py --policy normalized --task T03 --method RustCovenant-Verify --variant B2 --out ../replay-T03-B2
```

Expected process results are compile/SB/TB = PASS/PASS/PASS for fixed and B2, and PASS/ASSERT_OR_PANIC/ASSERT_OR_PANIC for B1. Some panic messages inside logs are deliberately caught; the final nonzero exit and uncaught assertion are what determine the B1 classification. StackVec T05 normalized obligation-guided fixed instead yields PASS/ERROR/ERROR because the generated harness does not compile; do not repair that program and overwrite its historical score.

The replay launcher checks Miri, bubblewrap, evaluator, and sysroot hashes against the archived context. A different binary distribution can fail this exact-context check even with the same nominal version. `--allow-runtime-drift` explicitly permits a diagnostic replay and records mismatches; such output is not pinned-runtime replication. Every replay generates new logs and a fresh locally signed record in its output directory. It makes no model request and never updates study scores. The official prior records used compiler commit 287487624357c19b22d27aa3ed584b8ccd080b4d, Miri seed 0, both SB/TB, and task-specific hidden-suite leak policy.

## Layout

| Path | Contents |
| --- | --- |
| `sources/` | Complete original, fixed, and candidate source snapshots, including upstream licenses and metadata. |
| `materials/` | Frozen C0, contracts, visible witnesses, ordinary tests, and reference hidden suites. |
| `generation/` | Exact prompts/responses and generated Rust tests. |
| `runs/reference/` | 14 finite reference executions. |
| `runs/strict/` | 28 original generated-suite executions. |
| `runs/sensitivity/` | 4 additional sensitivity executions; shared runs are stored once. |
| `study.json` | Identities, source hashes, cohort membership, and policy/run mappings. |
| `expected-comparison.json` | Expected generated-method totals, independently recomputed by `audit.py`. |
| `derivation.json` | Original/review hash pairs and path-normalization provenance for 11 logs. |
| `code/isolated_evaluator.py` | Original evaluator; `replay.py` overrides runtime locations. |
| `supplements/protocol/` | Finite protocol model and executable assertions. |

When strict parsing succeeds, both policies use the same `strict.rs`. Only T03 and T05 RustCovenant-Verify responses need a separate `normalized.rs`; their strict policy still reports GENERATION_ERROR. Every policy is checked against its original response and recorded test hash.

Protocol CSV/JSON reports are generated on demand. To retain them, run a copy outside the artifact:

```sh
cp -R supplements/protocol ../protocol-results
python3 -B ../protocol-results/test_protocol.py
```

## Preparing a GitHub repository

From this directory, initialize and stage the artifact:

```sh
git init
git add -- .
git add -f -- sources
python3 -B run_checks.py
```

The explicit source add preserves archived `Cargo.lock` files that upstream `.gitignore` rules would otherwise omit. `.gitattributes` disables line-ending conversion to preserve exact hashes. After pushing, run the audit on a fresh clone before sharing the repository.

## Provenance and licenses

This is an **unsigned review derivative**. Checksums establish package integrity, not independent authenticity. Original signed records and transport provenance remain in a separate restricted archive; see [REPRODUCIBILITY.md](REPRODUCIBILITY.md) for the transformation and evidence limits.

Upstream snapshots retain their original license and copyright files and manifest declarations. This package does not relicense third-party code.

# Third-Party Notices

`pkl-rs` is licensed under the [Apache License, Version 2.0](LICENSE). In addition to original code authored by OpenEdges Inc. and contributors, source and binary distributions of `pkl-rs` include or link the third-party components documented below.

---

## 1. Embedded Apple Pkl 0.32.0 Sources and Assets

- **Project**: [Apple Pkl](https://github.com/apple/pkl) (`0.32.0`, commit `e51fb2f91774265666f94980ec630db794a4db2d`)
- **License**: Apache License, Version 2.0
- **Copyright**: Copyright © 2024–2026 Apple Inc. and the Pkl project authors
- **Included components**:
  - `crates/pkl-runtime/src/stdlib/*.pkl`: All 23 `pkl:*` standard library modules (`base.pkl`, `analyze.pkl`, `Benchmark.pkl`, `Command.pkl`, `DocPackageInfo.pkl`, `DocsiteInfo.pkl`, `EvaluatorSettings.pkl`, `json.pkl`, `jsonnet.pkl`, `math.pkl`, `pklbinary.pkl`, `platform.pkl`, `Project.pkl`, `protobuf.pkl`, `ref.pkl`, `reflect.pkl`, `release.pkl`, `semver.pkl`, `settings.pkl`, `shell.pkl`, `test.pkl`, `xml.pkl`, `yaml.pkl`).
  - `crates/pkl-runtime/src/power_assertions.rs`, `diagnostic_values.rs`, `diagnostic/frames.rs`: Rust adaptations of upstream diagnostic, value rendering, and recursive frame compression logic.
  - `crates/pkl-doc/assets/`: `pkldoc.css`, `pkldoc.js`, `search-worker.js`, `favicon.svg`, `favicon-16x16.png`, `favicon-32x32.png`, `apple-touch-icon.png`.
  - `crates/pkl-cli/assets/completion_{bash,zsh,fish}.*`: Shell-completion scripts generated via [Clikt](https://github.com/ajalt/clikt) (Copyright © 2018 AJ Alt, Apache License 2.0).
  - `crates/pkl-host-native/assets/PklCARoots.pem`: Bundled CA root certificate bundle matching Apple Pkl `0.32.0`.

Upstream reference checkouts (`external/pkl`, `external/pkl-pantry`) are development/verification submodules and are not distributed inside prebuilt binary or npm release packages.

---

## 2. Third-Party Web Assets Bundled in `pkldoc`

The standalone `pkldoc` binary embeds static resources required to generate Pkl `0.32.0`-compatible HTML documentation sites (`crates/pkl-doc/assets/`):

### `scroll-into-view-if-needed` (`scroll-into-view.min.js`)

- **Project**: https://github.com/scroll-into-view/scroll-into-view-if-needed
- **License**: MIT License
- **Copyright**: Copyright (c) 2023 Cody Olsen

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Embedded Webfonts (`crates/pkl-doc/assets/fonts/`)

| Font family | Files | License |
| --- | --- | --- |
| **Lato** | `lato-v14-latin_latin-ext-{regular,700}.woff2` | SIL Open Font License 1.1 (OFL-1.1) |
| **Source Code Pro** | `source-code-pro-v7-latin_latin-ext-{regular,700}.woff2` | SIL Open Font License 1.1 (OFL-1.1) |
| **Open Sans** | `open-sans-v15-latin_latin-ext-{regular,italic,700,700italic}.woff2` | Apache License, Version 2.0 |
| **Material Icons** | `MaterialIcons-Regular.woff2` | Apache License, Version 2.0 |

---

## 3. Vendored Regular Expression Engine & Unicode Character Tables (`vendor/java_regex/`)

### `java_regex` (`0.1.0`, locally patched)

- **Project**: https://github.com/MathieuDutSik/java_regex
- **License**: `MIT OR Apache-2.0`
- **Copyright**: Copyright Mathieu Dutour Sikiric
- **Local modifications**: Documented in [vendor/java_regex/PKL-RS-PATCHES.md](vendor/java_regex/PKL-RS-PATCHES.md); full license texts preserved in `vendor/java_regex/LICENSE-APACHE` and `vendor/java_regex/LICENSE-MIT`.

### Unicode Character Database 15.0.0

- **Project**: The Unicode Standard, Version 15.0.0 (https://www.unicode.org/)
- **License**: Unicode, Inc. License Agreement — Data Files and Software (`Unicode-3.0`)
- **Copyright**: Copyright © 1991–2022 Unicode, Inc. All rights reserved.
- **Full notice**: Preserved verbatim in [vendor/java_regex/jdk-character-names-legal/unicode.md](vendor/java_regex/jdk-character-names-legal/unicode.md).

### OpenJDK `uniName.dat` Provenance Notice (Pre-GA Legal Review Item)

- Character name lookup tables in `vendor/java_regex` were deterministically decoded from `java.base/java/lang/uniName.dat` in Eclipse Temurin JDK `21.0.12.1+1` (derived from Unicode 15.0 `UnicodeData.txt`), as documented in [vendor/java_regex/JDK-CHARACTER-NAMES.md](vendor/java_regex/JDK-CHARACTER-NAMES.md).
- The accompanying OpenJDK legal notices (`LICENSE` [GPLv2], `ASSEMBLY_EXCEPTION`, `ADDITIONAL_LICENSE_INFO` [Classpath Exception], and `unicode.md`) are preserved in `vendor/java_regex/jdk-character-names-legal/`.
- **Release compliance note**: Prior to general availability (GA) public distribution under pure Apache-2.0, regenerating the character name table directly from the official Unicode Consortium `UnicodeData.txt` (pure Unicode License Agreement, removing OpenJDK `uniName.dat` provenance) is recommended to eliminate any GPLv2+Classpath-Exception licensing ambiguity.

---

## 4. Rust Crate Dependencies (`Cargo.lock`)

All **83 external Rust crates** locked in [Cargo.lock](Cargo.lock) are distributed under permissive open-source licenses (zero copyleft crates):

| SPDX License Expression | Crate Count | Representative Crates |
| --- | ---: | --- |
| `MIT OR Apache-2.0` / `Apache-2.0 OR MIT` / `MIT/Apache-2.0` | 62 | `rustls`, `napi`, `wasm-bindgen`, `js-sys`, `sha1`, `sha2`, `md-5`, `libm`, `libc`, `syn`, `quote`, `time`, `semver`, `unicode-*`, `windows-*` |
| `MIT` | 9 | `pem`, `zeroize`, `bumpalo`, `convert_case`, `shlex`, `yasna`, `nohash-hasher`, `rcgen` (dev-only) |
| `ISC` | 3 | `libloading`, `rustls-webpki`, `untrusted` |
| `Apache-2.0 OR ISC OR MIT` / `Apache-2.0 AND ISC` | 3 | `ring`, `rustls-pki-types`, `rustls-pemfile` |
| `Zlib OR Apache-2.0 OR MIT` | 2 | `tinyvec`, `tinyvec_macros` |
| `Apache-2.0` | 1 | `proc-macro2` / `wasi` |
| `BSD-3-Clause` | 1 | `subtle` |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 | `unicode-ident` |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 1 | `wasi` |

To inspect the exact per-crate SPDX metadata locally:

```sh
cargo metadata --format-version 1 --locked
```

---

## 5. Historical Test Fixtures (Development Repository Only)

- `tests/fixtures/rev1/`: Imported regression fixtures retaining original `LICENSE` (Apache-2.0) and `NOTICE` (OpenEdges Inc. 2025; Apple Inc. and Pkl project authors 2024–2025). Not shipped in binary or npm distributions.

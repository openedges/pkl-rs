# Third-Party Notices

`pkl-rs` is licensed under the [Apache License, Version 2.0](LICENSE) (Copyright 2025–2026 OpenEdges Inc.).

> **Trademark & Non-Affiliation Notice**: `pkl-rs` is an independent open-source project maintained by OpenEdges Inc. and is not affiliated with, sponsored by, or endorsed by Apple Inc. "Pkl" is a trademark or product name of Apple Inc., referenced solely to describe language and specification compatibility. All `pkldoc` favicon assets under `crates/pkl-doc/assets/images/` are original OpenEdges Inc. artwork licensed under Apache-2.0.

In addition to original code authored by OpenEdges Inc. and contributors, source and binary distributions of `pkl-rs` include or link the third-party components and licenses listed below.

---

## 1. Embedded Apple Pkl 0.32.0 Sources and Assets

- **Project**: [Apple Pkl](https://github.com/apple/pkl) (`0.32.0`, commit `e51fb2f91774265666f94980ec630db794a4db2d`)
- **License**: Apache License, Version 2.0
- **Copyright**: Copyright © 2024–2026 Apple Inc. and the Pkl project authors
- **Included / adapted components**:
  - `crates/pkl-runtime/src/stdlib/*.pkl`: All 23 `pkl:*` standard library modules (`base.pkl`, `analyze.pkl`, `Benchmark.pkl`, `Command.pkl`, `DocPackageInfo.pkl`, `DocsiteInfo.pkl`, `EvaluatorSettings.pkl`, `json.pkl`, `jsonnet.pkl`, `math.pkl`, `pklbinary.pkl`, `platform.pkl`, `Project.pkl`, `protobuf.pkl`, `ref.pkl`, `reflect.pkl`, `release.pkl`, `semver.pkl`, `settings.pkl`, `shell.pkl`, `test.pkl`, `xml.pkl`, `yaml.pkl`), copied verbatim with original Apache-2.0 headers preserved.
  - Rust adaptations of upstream diagnostic, value rendering, ANSI styling, syntax highlighting, recursive stack-frame compression, lexical path resolution, and YAML scalar tag resolution algorithms:
    - `crates/pkl-runtime/src/power_assertions.rs`
    - `crates/pkl-runtime/src/diagnostic_values.rs`
    - `crates/pkl-runtime/src/diagnostic/ansi.rs`
    - `crates/pkl-runtime/src/diagnostic/highlight.rs`
    - `crates/pkl-runtime/src/diagnostic/frames.rs`
    - `crates/pkl-runtime/src/vm/settings_paths.rs`
    - `crates/pkl-runtime/src/vm/yaml_parser/backend/resolve.rs`
  - Functional `pkldoc` web resources under `crates/pkl-doc/assets/`: `styles/pkldoc.css`, `scripts/pkldoc.js`, `scripts/search-worker.js`, and fonts listed in Section 5. *(All four icon files under `crates/pkl-doc/assets/images/` are original OpenEdges Inc. Apache-2.0 artwork, not Apple Pkl logos.)*
  - `crates/pkl-cli/assets/completion_{bash,zsh,fish}.*`: Shell-completion scripts generated via [Clikt](https://github.com/ajalt/clikt) (Copyright © 2018 AJ Alt, Apache License 2.0).
  - `crates/pkl-host-native/assets/PklCARoots.pem`: Bundled CA root certificate bundle matching Apple Pkl `0.32.0`.

Upstream reference checkouts (`external/pkl`, `external/pkl-pantry`) are development/verification submodules and are not distributed inside prebuilt binary or npm release packages.

---

## 2. Sun Microsystems FDLIBM (Math Kernels)

- **Files**: `crates/pkl-runtime/src/numeric.rs`, `crates/pkl-runtime/src/vm/math_fdlibm.rs`
- **Source**: FDLIBM (`s_cbrt.c`, `e_asin.c`, `e_acos.c`, `s_atan.c`, `e_atan2.c`, `e_log.c`, `e_log10.c`, and trigonometric kernels, https://netlib.org/fdlibm/)

```text
Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.

Developed at SunPro / SunSoft, a Sun Microsystems, Inc. business.
Permission to use, copy, modify, and distribute this software is freely
granted, provided that this notice is preserved.
```

---

## 3. SnakeYAML Engine 3.0.1

- **Project**: [SnakeYAML Engine](https://bitbucket.org/snakeyaml/snakeyaml-engine) (`3.0.1`)
- **License**: Apache License, Version 2.0
- **Copyright**: Copyright (c) 2018, SnakeYAML
- **Files**: Adapted in `crates/pkl-runtime/src/vm/yaml_parser/backend/` (`PROVENANCE.md` and `LICENSE-APACHE-2.0.txt` preserved alongside the Rust adaptation).

---

## 4. Android Open Source Project / Apache Spark TimSort

- **Project**: [Android Open Source Project](https://source.android.com/) / [Apache Spark](https://spark.apache.org/) (`v3.5.7` `TimSort.java`)
- **License**: Apache License, Version 2.0
- **Copyright**: Copyright (C) 2008 The Android Open Source Project
- **Files**: Adapted in `crates/pkl-runtime/src/vm/protobuf_spark_sort.rs`.

---

## 5. Third-Party Web Assets Bundled in `pkldoc`

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
OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE SOFTWARE.
```

### Embedded Webfonts (`crates/pkl-doc/assets/fonts/`)

| Font family | Files | License |
| --- | --- | --- |
| **Lato** | `lato-v14-latin_latin-ext-{regular,700}.woff2` | SIL Open Font License 1.1 (OFL-1.1) |
| **Source Code Pro** | `source-code-pro-v7-latin_latin-ext-{regular,700}.woff2` | SIL Open Font License 1.1 (OFL-1.1) |
| **Open Sans** | `open-sans-v15-latin_latin-ext-{regular,italic,700,700italic}.woff2` | Apache License, Version 2.0 |
| **Material Icons** | `MaterialIcons-Regular.woff2` | Apache License, Version 2.0 |

#### SIL Open Font License, Version 1.1 (Applies to Lato & Source Code Pro)

```text
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

---

## 6. Embedded Regular Expression Engine & Official Unicode 15.0.0 Data (`crates/pkl-runtime/src/java_regex/`)

### `java_regex` (`0.1.0`, adapted in `crates/pkl-runtime/src/java_regex/`)

- **Project**: https://github.com/MathieuDutSik/java_regex
- **License**: `MIT OR Apache-2.0` (MIT license text included in Section 5 above; Apache-2.0 text in [LICENSE](LICENSE))
- **Copyright**: Copyright Mathieu Dutour Sikiric
- **Local modifications**: Adapted into a self-contained raw UTF-16 `java.util.regex.Pattern` matcher inside `crates/pkl-runtime/src/java_regex/` with explicit step/depth execution budgets, canonical equivalence (`CANON_EQ` / `(?c)`), and zero external regex or Unicode crate dependencies.

### Official Unicode 15.0.0 Character Database (`Unicode-3.0`)

- **Project**: The Unicode Standard, Version 15.0.0 (https://www.unicode.org/Public/15.0.0/ucd/)
- **Generation**: All 12 data tables under `crates/pkl-runtime/src/java_regex/` are deterministically generated directly from official Unicode Consortium UCD 15.0.0 files by `tools/generate_ucd15_regex_tables.py` (exact SHA-256 provenance recorded in [crates/pkl-runtime/src/java_regex/UNICODE-15.0.0-PROVENANCE.md](crates/pkl-runtime/src/java_regex/UNICODE-15.0.0-PROVENANCE.md)). No OpenJDK binary resources (`uniName.dat`) or GPLv2 files are used or distributed.
- **License**: `Unicode-3.0` (verbatim text preserved in [crates/pkl-runtime/src/java_regex/LICENSE-UNICODE](crates/pkl-runtime/src/java_regex/LICENSE-UNICODE)):

```text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2026 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.
```

---

## 7. Statically Linked Rust Crate Dependencies (`Cargo.lock`)

All **79 external Rust crates** locked in [Cargo.lock](Cargo.lock) (**67 runtime/build crates** across shipped binaries/packages; **zero copyleft crates**) are distributed under permissive open-source licenses:

| SPDX License Expression | Crate Count | Representative Crates |
| --- | ---: | --- |
| `MIT OR Apache-2.0` / `Apache-2.0 OR MIT` / `MIT/Apache-2.0` | 60 | `rustls`, `wasm-bindgen`, `js-sys`, `sha1`, `sha2`, `md-5`, `libc`, `syn`, `quote`, `time`, `semver`, `unicode-normalization`, `windows-*` |
| `MIT` | 8 | `napi`, `napi-derive`, `napi-sys`, `libm`, `convert_case`, `pem`, `zeroize`, `bumpalo` |
| `ISC` | 3 | `libloading`, `rustls-webpki`, `untrusted` |
| `Apache-2.0 OR ISC OR MIT` / `Apache-2.0 AND ISC` | 3 | `ring`, `rustls-pki-types`, `rustls-pemfile` |
| `Zlib OR Apache-2.0 OR MIT` | 2 | `tinyvec`, `tinyvec_macros` |
| `BSD-3-Clause` | 1 | `subtle` |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 | `unicode-ident` |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 1 | `wasi` |

### `ring` (`0.17.14`, `Apache-2.0 AND ISC`)

`ring` includes code derived from BoringSSL (Copyright Google Inc. and contributors, ISC-style / Apache-2.0) and portions of OpenSSL (Copyright 1995–2016 The OpenSSL Project / Eric Young, permissive OpenSSL/SSLeay terms). This product includes software developed by the OpenSSL Project for use in the OpenSSL Toolkit (http://www.openssl.org/) and cryptographic software written by Eric Young (eay@cryptsoft.com).

### ISC License (Applies to `libloading`, `rustls-webpki`, `untrusted`, and `ring`)

```text
Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

### BSD-3-Clause License (Applies to `subtle`)

```text
Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

To inspect exact per-crate SPDX metadata locally:

```sh
cargo metadata --format-version 1 --locked
```

---

## 8. Historical Test Fixtures (Development Repository Only)

- `tests/fixtures/rev1/`: Imported regression fixtures retaining original `LICENSE` (Apache-2.0) and `NOTICE` (OpenEdges Inc. 2025; Apple Inc. and Pkl project authors 2024–2025).
- `tests/fixtures/`: Compatibility verification fixtures derived from Apple Pkl `0.32.0` (Apache-2.0). Test fixtures are excluded from binary and npm release packages.

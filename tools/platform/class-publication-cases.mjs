// Pinned Java 0.32.0 same-evaluator captures at file:///m2-platform.pkl.
// Acquisition: target/m2/stage12-java-observations-03, repetition 1.
// Expected payloads preserve every stdout/stderr byte; Rust never supplies them.
export const classPublicationOracle = {
  "acquisition": "target/m2/stage12-java-observations-03",
  "reportSha256": "0df3cdbcbcf3ea8a4a99c7c77d33ce82e28347307b2e5d70e52d85018aeaffde",
  "oracleVersion": "0.32.0",
  "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
  "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
  "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
  "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false,
  "repetition": 1
};

export const classPublicationCases = [
  {
    "id": "prefix",
    "source": "open class Base { fixed value: Int = 42 }\nclass Child extends Base { early: Int = 10; value: Int = 1; later: Int = 99 }\ntrigger = Child == Child\ninherited = new Child {}.value\nearly = new Child {}.early\nlater = new Child {}.later\nassigned = new Child { value = 1 }.value\nfreshAssigned = new Child { fresh = 77 }.fresh\ntailAssigned = new Child { later = 88 }.later\nidentity = Child == Child\nordinary = throw(\"ordinary failure\")\n",
    "expressions": [
      "trigger",
      "trigger",
      "inherited",
      "early",
      "later",
      "assigned",
      "freshAssigned",
      "tailAssigned",
      "identity",
      "ordinary",
      "ordinary"
    ],
    "expected": [
      "error\t–– Pkl Error ––\nMissing modifier `fixed`.\n\n2 | class Child extends Base { early: Int = 10; value: Int = 1; later: Int = 99 }\n                                                ^^^^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nProperty `value` must be declared fixed, because it overrides a fixed property on parent class `m2-platform#Base`.\n\n3 | trigger = Child == Child\n              ^^^^^\nat m2-platform#trigger (file:///m2-platform.pkl)\n\n1 | trigger\n    ^^^^^^^\nat  (repl:text)\n",
      "value\ttrue",
      "value\t42",
      "value\t10",
      "error\t–– Pkl Error ––\nCannot find property `later` in object of type `m2-platform#Child`.\n\n6 | later = new Child {}.later\n            ^^^^^^^^^^^^^^^^^^\nat m2-platform#later (file:///m2-platform.pkl)\n\nAvailable properties:\nearly\nvalue\n\n1 | later\n    ^^^^^\nat  (repl:text)\n",
      "error\t–– Pkl Error ––\nCannot assign to fixed property `value`.\n\n7 | assigned = new Child { value = 1 }.value\n                           ^^^^^\nat m2-platform#assigned (file:///m2-platform.pkl)\n\n1 | assigned\n    ^^^^^^^^\nat  (repl:text)\n",
      "value\t77",
      "value\t88",
      "value\ttrue",
      "error\t–– Pkl Error ––\nordinary failure\n\n11 | ordinary = throw(\"ordinary failure\")\n                ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#ordinary (file:///m2-platform.pkl)\n\n1 | ordinary\n    ^^^^^^^^\nat  (repl:text)\n",
      "error\t–– Pkl Error ––\nordinary failure\n\n11 | ordinary = throw(\"ordinary failure\")\n                ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#ordinary (file:///m2-platform.pkl)\n\n1 | ordinary\n    ^^^^^^^^\nat  (repl:text)\n"
    ],
    "sourceSha256": "1a2510e220a775ffac0c7c36dc63f9241cfc90de66c42536eddf1567ba8082a2",
    "captureSha256": {
      "0.stderr.bin": "a19e6fd39a31edf6589e359b85e325b919b44e055622e62cac44fc05d9a5b4ca",
      "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "2.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
      "3.stdout.bin": "4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5",
      "4.stderr.bin": "26bebe0b5473299c7f4a3416477ab078722f796bc233f77aa6ffbb0d2748f2b2",
      "5.stderr.bin": "ec48b770c6425451ec6a2d6d1f5ee3f5a7e758dd1b86c908c090ce86cdf93591",
      "6.stdout.bin": "a88a7902cb4ef697ba0b6759c50e8c10297ff58f942243de19b984841bfe1f73",
      "7.stdout.bin": "8b940be7fb78aaa6b6567dd7a3987996947460df1c668e698eb92ca77e425349",
      "8.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "9.stderr.bin": "4ececca00cdf1ba6c83d9e02db32719a45c4b5908dec6d2536839c2357dca0cd",
      "10.stderr.bin": "4ececca00cdf1ba6c83d9e02db32719a45c4b5908dec6d2536839c2357dca0cd"
    }
  },
  {
    "id": "method-prefix",
    "source": "class Child { function before(): Int = 7; early: Int = 10; function bad(x: Missing): Int = 8; later: Int = 99; function after(): Int = 9 }\ntrigger = Child == Child\ninherited = new Child {}.value\nearly = new Child {}.early\nlater = new Child {}.later\nassigned = new Child { value = 1 }.value\nfreshAssigned = new Child { fresh = 77 }.fresh\ntailAssigned = new Child { later = 88 }.later\nidentity = Child == Child\nordinary = throw(\"ordinary failure\")\nbefore = new Child {}.before()\nafter = new Child {}.after()\n",
    "expressions": [
      "trigger",
      "trigger",
      "early",
      "later",
      "before",
      "after",
      "freshAssigned"
    ],
    "expected": [
      "error\t–– Pkl Error ––\nCannot find type `Missing`.\n\n1 | class Child { function before(): Int = 7; early: Int = 10; function bad(x: Missing): Int = 8; later: Int = 99; function after(): Int = 9 }\n                                                                               ^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n2 | trigger = Child == Child\n              ^^^^^\nat m2-platform#trigger (file:///m2-platform.pkl)\n\n1 | trigger\n    ^^^^^^^\nat  (repl:text)\n",
      "value\ttrue",
      "value\t10",
      "value\t99",
      "value\t7",
      "error\t–– Pkl Error ––\nCannot find method `after` in class `m2-platform#Child`.\n\n12 | after = new Child {}.after()\n             ^^^^^^^^^^^^^^^^^^^^\nat m2-platform#after (file:///m2-platform.pkl)\n\nDid you mean any of the following?\nlater\n\n1 | after\n    ^^^^^\nat  (repl:text)\n",
      "value\t77"
    ],
    "sourceSha256": "61058150fd1348d0114b0e1976fdfe8b9f349d2b8628e3b897902941bb234e1c",
    "captureSha256": {
      "0.stderr.bin": "c91b849803f6b95c9588a50bd426d749caa77523f15cf97511d257b21f3ba000",
      "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "2.stdout.bin": "4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5",
      "3.stdout.bin": "8c1f1046219ddd216a023f792356ddf127fce372a72ec9b4cdac989ee5b0b455",
      "4.stdout.bin": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "5.stderr.bin": "fb0d86efc0078a905c97fc441256c3b9559366c3262f61367d74e64bc2a7207e",
      "6.stdout.bin": "a88a7902cb4ef697ba0b6759c50e8c10297ff58f942243de19b984841bfe1f73"
    }
  },
  {
    "id": "recursive-type",
    "source": "class Child { self: Child?; early: Int = 10 }\ntrigger = Child == Child\ninherited = new Child {}.value\nearly = new Child {}.early\nlater = new Child {}.later\nassigned = new Child { value = 1 }.value\nfreshAssigned = new Child { fresh = 77 }.fresh\ntailAssigned = new Child { later = 88 }.later\nidentity = Child == Child\nordinary = throw(\"ordinary failure\")\nselfNull = new Child {}.self == null\n",
    "expressions": [
      "trigger",
      "trigger",
      "selfNull",
      "early",
      "identity"
    ],
    "expected": [
      "value\ttrue",
      "value\ttrue",
      "value\ttrue",
      "value\t10",
      "value\ttrue"
    ],
    "sourceSha256": "0baa2a56f5c6da64b559baef3b039187b4ff7e2688d9cc06ce0d0be44da37ccb",
    "captureSha256": {
      "0.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "2.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "3.stdout.bin": "4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5",
      "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b"
    }
  },
  {
    "id": "wrapper-retry",
    "source": "open class Base { fixed value: Int = 42 }\nclass Child extends Base { early: Int = 10; value: Int = 1; later: Int = 99 }\ninvalid = Child\ntrigger = invalid == invalid\ninherited = new Child {}.value\nearly = new Child {}.early\nidentity = invalid == Child\nordinary = throw(\"ordinary failure\")\n",
    "expressions": [
      "trigger",
      "trigger",
      "inherited",
      "early",
      "identity",
      "ordinary",
      "ordinary"
    ],
    "expected": [
      "error\t–– Pkl Error ––\nMissing modifier `fixed`.\n\n2 | class Child extends Base { early: Int = 10; value: Int = 1; later: Int = 99 }\n                                                ^^^^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nProperty `value` must be declared fixed, because it overrides a fixed property on parent class `m2-platform#Base`.\n\n3 | invalid = Child\n              ^^^^^\nat m2-platform#invalid (file:///m2-platform.pkl)\n\n4 | trigger = invalid == invalid\n              ^^^^^^^\nat m2-platform#trigger (file:///m2-platform.pkl)\n\n1 | trigger\n    ^^^^^^^\nat  (repl:text)\n",
      "value\ttrue",
      "value\t42",
      "value\t10",
      "value\ttrue",
      "error\t–– Pkl Error ––\nordinary failure\n\n8 | ordinary = throw(\"ordinary failure\")\n               ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#ordinary (file:///m2-platform.pkl)\n\n1 | ordinary\n    ^^^^^^^^\nat  (repl:text)\n",
      "error\t–– Pkl Error ––\nordinary failure\n\n8 | ordinary = throw(\"ordinary failure\")\n               ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#ordinary (file:///m2-platform.pkl)\n\n1 | ordinary\n    ^^^^^^^^\nat  (repl:text)\n"
    ],
    "sourceSha256": "265ccb4d69dc66fb27b3763c20c1a97ae60b28859c98a59dbab382a055674229",
    "captureSha256": {
      "0.stderr.bin": "2c1520aa8ca9de9afa9fdd521a9cbc00644d699bb62ca1108e280260e900913a",
      "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "2.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
      "3.stdout.bin": "4a44dc15364204a80fe80e9039455cc1608281820fe2b24f1e5233ade6af1dd5",
      "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "5.stderr.bin": "d6dd76abdb4f6b462d86fc333f14b342e13e852940c51399c4de34a604df274b",
      "6.stderr.bin": "d6dd76abdb4f6b462d86fc333f14b342e13e852940c51399c4de34a604df274b"
    }
  }
];

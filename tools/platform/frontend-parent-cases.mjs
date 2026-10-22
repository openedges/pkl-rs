// Fresh pinned Java 0.32.0 complete payloads at the identical module URI.
// Calls use fresh evaluators; frontend rejection and accepted lazy controls are distinct.
export const frontendParentContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false,
  "oracle": {
    "acquisitions": {
      "target/m2/stage33-oracle-01": {
        "oracleSha256": "064836920a3c5089197317276fd3e2ea19a8b6c35330a19e7ccf6bea642ab6ec",
        "reportSha256": "fcee18f230cf13425e964493a18e3054d3dd2b128d5eb13602b3a19b7ea10f76"
      },
      "target/m2/stage33-oracle-02": {
        "oracleSha256": "064836920a3c5089197317276fd3e2ea19a8b6c35330a19e7ccf6bea642ab6ec",
        "reportSha256": "94b7f3a87a003550321118709d6149d3a38cb99b39631e5385e43f29f94394c8"
      },
      "target/m2/stage33-oracle-03": {
        "oracleSha256": "064836920a3c5089197317276fd3e2ea19a8b6c35330a19e7ccf6bea642ab6ec",
        "reportSha256": "b580e9e6c4473075e07384c4f4b0dba26f839547241924e1ff1e1cf3191a751b"
      }
    },
    "selectionPath": "target/m2/stage33-selected-provenance-01/selection.json",
    "selectionSha256": "42acb3bf9abf7ee494973011d1738a350a12e17b489ff644e98a109f4ae2cd7e",
    "payloadSha256": "96ba963433d6fd70ffa387ce61d99434d7033073e423bf1f97d27a5bc3765688",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf"
  }
};

export const frontendParentCases = [
  {
    "id": "diagnostic-stage33-module-function",
    "source": "function build() = new { bad: Int = 1 }\nresult = build().bad\n",
    "expression": "result",
    "sourceSha256": "2db0c8191468ce9b491750039a6b0cb73c932002f13cd374b7d54da14499ec43",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/module-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7181cd91f4724231be927d25be96d3445393963f0ac37820c8c260eabb008409",
      "process": "9d4fe37e07168bdad90979768b3530510dac010c0beabd0d596ae81b19e0b1a0"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function build() = new { bad: Int = 1 }\n                                  ^^^\nat m2-platform#build.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-lambda",
    "source": "f = () -> new { bad: Int = 1 }\nresult = f().bad\n",
    "expression": "result",
    "sourceSha256": "7a24a6f3a4a2cbea0a9ee3bdd8abdbc58f480bae48741866557ce3b11ca1919d",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/lambda",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b2c754b6e7d45b3bd75283265a946b90e96cbdb565fc3310e8ce6f0647c983c2",
      "process": "988d7d57360b618272c6d72cae132f927bb71070c43d08bcbcd7aea9a4f6699b"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | f = () -> new { bad: Int = 1 }\n              ^^^\nat m2-platform#f.<function#1> (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "diagnostic-stage33-unused-invalid-function",
    "source": "function unused() = new { bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "66c8986a31bfebbf0cec9922f8063be1f16803b7ce9352373b2bd294491ee7f8",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/unused-invalid-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e0005bae25257eb42a6665d084ee5321b33e8b7321be038e5585cb5469bb4157",
      "process": "53165f4ec6413f8b51f9ba12fbd1fb7fa8d4cec4c726eabd16c3346e94d27702"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function unused() = new { bad: Int = 1 }\n                                   ^^^\nat m2-platform#unused.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-annotation",
    "source": "x = new { const bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "d2d048e5eb9e0a7c9f354b0061215e8fa81a8ff2064403b3b9c19cf6d09edaa3",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-annotation",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "aef87a689d72046df423160a40c0827cb7e2ed068b401cb501ed23e0e851865c",
      "process": "f37da7415c02c459e4babc75b7dd3d979074261497218fba5d2b886310bc0a82"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | x = new { const bad: Int = 1 }\n              ^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-object-local-function",
    "source": "x = new { local function build() = new { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "820b1c16b78fd9dd1e3d15e526beba86bf9767d8d2aef64804642498b7a7b491",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/object-local-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3021542c65911eef75269ee9301ecae5678e9d19123e24ff66b925b9dccdc571",
      "process": "835931a6ddab319fa0f71d322bceda71ba979034d0a2d9e71834bc83c21b6af3"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { local function build() = new { bad: Int = 1 } }\n                                                  ^^^\nat m2-platform#x.build.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-negative",
    "source": "x = new { const bad: Int = -1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "0742fdaa8273ee0f7e9c2aaf5480dbeffcaf5284d6b79316b8c14c7bd5bb5675",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-negative",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "2d9f3bcfdc5d08f6710dd944d908c5c232ced8e8f107ec00258951f9b4c4ffcf",
      "process": "69ca47ae9301d531a948155765b4670636e59267f8ac5189926b19cb0a9fe213"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | x = new { const bad: Int = -1 }\n              ^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-nested",
    "source": "x = new { child = new { const bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "e13e6ca5f9997df7fcd9fc846c7a0433c0d5ede1b645fb37080d1b795aafccfd",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-nested",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "300c2e5c9c5eec80ebbce6432f279a0a898179a32aed6899ee6c6183804b5999",
      "process": "97c99029318bb4e64b4f56a801d5cec2c64b9e1e7111fe44b8f7e8e3511cd9f1"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | x = new { child = new { const bad: Int = 1 } }\n                            ^^^^^\nat m2-platform#x.child (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-quoted",
    "source": "`outer odd` = new { const `bad odd`: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "d2ab8e2a579f42f0bee9b7f5beab43c0baeb4c501ca2b63c7adf382629383749",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-quoted",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "420912fa8a84606c0fe2c346d8001ddbf581a164d89d30fbdb267a8c0b7a5494",
      "process": "1bb97023f3beb82e4c857b329efa70c8a9f8080435cef67f48be16f5742fb7f6"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | `outer odd` = new { const `bad odd`: Int = 1 }\n                        ^^^^^\nat m2-platform#`outer odd` (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-function",
    "source": "function build() = new Dynamic { const bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "282c289900406ad1091d88cc330dd375af3f75d096ebfa8f3953d040386da39a",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "73d17448d4223120f2765d645626ef0ff3cbb93219fff004fed685ae8e3299fd",
      "process": "a552c02383aba567e25be41d9417d8117b0589f39c8de10427a13304f274e4ba"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | function build() = new Dynamic { const bad: Int = 1 }\n                                     ^^^^^\nat m2-platform#build (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-lambda",
    "source": "f = () -> new Dynamic { const bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "afe14251c3d712f6f9da724c5e631220e25a74bd60e787ab5b06e6b24c955424",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-lambda",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8137019aefe9ab5c0280a72bc9852bde8c9dd0ae0f1d9663e9c49c27aa38e05f",
      "process": "bd9320dcd48223cec60d79033ffd792bfb30cecc509cf926ef8753c64a7a5ce2"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | f = () -> new Dynamic { const bad: Int = 1 }\n                            ^^^^^\nat m2-platform#f.<function#1> (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-let",
    "source": "x = let (a = 1) new { const bad: Int = a }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "f2192d16325a177c9ad2dd6af1536ef844e817e8f668835a830b844ec07f2452",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-let",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a6de389221886edeeaee27d34e4b95dd87574d53f1ce070a7caf4c550ba70716",
      "process": "2404a8a4b2054f0ebab1c8a942525b366544853c3ea7c7070c3fdceb24c4f4ab"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | x = let (a = 1) new { const bad: Int = a }\n                          ^^^^^\nat m2-platform#x.<let expr> (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-entry",
    "source": "x = new Mapping { [\"entry\"] = new { const bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "a7902cd7f600098d9e0b534e87a8bec5007d4463b8a4a53d84cae39145a3d823",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-entry",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "097e835d5f388ae9bad9cc24750730220073ca67380a8d2d9445d93c55e828e4",
      "process": "a218abb209be5e122995845ad7aa2930ad9c3fa7f189dd8f46a1d351bcd14434"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | x = new Mapping { [\"entry\"] = new { const bad: Int = 1 } }\n                                        ^^^^^\nat m2-platform#x[\"entry\"] (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-earlier-error",
    "source": "x = new { const bad: Int = \"\\q\" }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "de935ce973c3ecdf2f87b5cae3a1a9b5940b17e8a0b72a2470a553161372c15d",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-earlier-error",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7fcec292a3d96457cd1dd79126adc01ea37d06a4f4457ff213361b70684d50af",
      "process": "a96491c11db28af6d40ed8f1ff8b85b056ea62ec4215d5a5116a13dec30468ba"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | x = new { const bad: Int = \"\\q\" }\n                                ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-const-untyped",
    "source": "x = new { const bad = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "23c8589384b48b1a984aa5226742556aaedac015806aa0c59ab0b7c00548a945",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/const-untyped",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "94f81a4e17d3dc7ff64fc86bf13c25d41286d1c9016d024d3a1859ca5b716898",
      "process": "f4f11d4f31e9bf3e41068c2e3bde255cb5c5c4a0e8b07ad49fa48309832c5676"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | x = new { const bad = 1 }\n              ^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-local-const-control",
    "source": "x = new { local const good: Int = 1; result = good }\nresult = x.result\n",
    "expression": "result",
    "sourceSha256": "3f56d70f2e461f2c7dd5dc1206721d2a55692b86a60b0e9dcec1c32cba43b4ec",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/local-const-control",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "7fdf4cee37ea4ae12cafccd2cf0ae4675cc98532bb5edc6053d0ca572798b3ca"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage33-class-const-control",
    "source": "class C { const good: Int = 1 }\nresult = new C {}.good\n",
    "expression": "result",
    "sourceSha256": "f673a3598776f2db46cbb29921d74c70672481ea2d8e45b950802ffb2bf3632a",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/class-const-control",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "5e923aa71ec6729220178fc42d892195886fee458a682ce8b7894182fc797340"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage33-module-const-control",
    "source": "const good: Int = 1\nresult = good\n",
    "expression": "result",
    "sourceSha256": "53faa747c1e7745b791139c77376bf33615493e43e4caee0a95551711e0b13ca",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/module-const-control",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "ef8e1467d7dc23d9ce7efe1e451f272de5ab5868d0aacfb499cc704656c5ecc5"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage33-lambda-dynamic",
    "source": "f = () -> new Dynamic { bad: Int = 1 }\nresult = f().bad\n",
    "expression": "result",
    "sourceSha256": "d06e8e897b1673ef34244e63a8b28239ea9f243db6de8cf1b8f9d61a861018d8",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/lambda-dynamic",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "515e7cf8a53e7ecc0a23e771eff887df39fedd7c08d56f2afae96e77d1f6bb30",
      "process": "4f6fccf6f5978eba8d72b78db80a7b20460d52ecd09a54e695582241f18a9b9e"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | f = () -> new Dynamic { bad: Int = 1 }\n                                 ^^^\nat m2-platform#f.<function#1>.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-object-local-function-dynamic",
    "source": "x = new { local function build() = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "a07805c69cd055c3fd81407f96cedddddc7f69b0ce7ef28d064eb338b4165dc0",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/object-local-function-dynamic",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ea81a81eaa9266030a1f95f2bc46f86a82e9c4ec9bb82cb20f941347482759fd",
      "process": "c93acfbd48458a0dda18a48589affc12ecfb9c6d72fe7f95f4b68932d73298ec"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { local function build() = new Dynamic { bad: Int = 1 } }\n                                                          ^^^\nat m2-platform#x.build.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-function-return-type",
    "source": "function build(): Dynamic = new { bad: Int = 1 }\nresult = build().bad\n",
    "expression": "result",
    "sourceSha256": "313bc14c27b54ad7e68fd4e5ac06019e7f5988489df2e0b727e63256c1ae5961",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-return-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "fdce8432d845ecfcb19168bea713a9d9487e97c0ded2a7714917be88c9807f4e",
      "process": "7e70c607c05c927f28e58d77b4a6bed2a8a5bb05a2d240e0d0b420889f355ee1"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function build(): Dynamic = new { bad: Int = 1 }\n                                           ^^^\nat m2-platform#build.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-function-valid-inferred",
    "source": "function build() = new { good = 1 }\nresult = build().good\n",
    "expression": "result",
    "sourceSha256": "29ee5f486552b93c437229382db6fbe2bbe7a319a9cb7bedff1d058a0d0793df",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-valid-inferred",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "3669d17ce5eab05ed9258d469c3d17dda7d9399b613d643b05a1b572fa13e40a"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage33-lambda-valid-inferred",
    "source": "f = () -> new { good = 1 }\nresult = f().good\n",
    "expression": "result",
    "sourceSha256": "8fb38dd26de90c745c75decce414b765dc5c6f7d57a6df7fa5e5b0d77cec4dc3",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/lambda-valid-inferred",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "278144c602a230ec72e3ad483ce503c69a75d9a01a72c9f5350544950cd0e4d1",
      "process": "eda218464a4119167dd987f7234048752aadd32dce6ae5ef95516ad62eaa1b54"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | f = () -> new { good = 1 }\n              ^^^\nat m2-platform#f.<function#1> (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "diagnostic-stage33-function-nested-inferred",
    "source": "function build() = new { child = new { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "fd5c575d8d8770a782590da194b4565e5e833be1f8f6123e8833c752b61ed84c",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-nested-inferred",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "268e7ae2ede33c6ec4b84e26fbefb4da4efc6be4ee1a7f6123f74c5d32be8e3d",
      "process": "1cedb2699eb735e3d1e0039d94702c44163ea580904fa5ed18b2dccafa511e4a"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function build() = new { child = new { bad: Int = 1 } }\n                                                ^^^\nat m2-platform#build.child.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-lambda-quoted-inferred",
    "source": "`odd lambda` = () -> new { `bad odd`: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "58d1217c0dfb46d0a70047369c7b6a42825f68ac8ecb333b1adfc93a9d953af7",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/lambda-quoted-inferred",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "74b5ff1b834d13ec9a1dac56cfe45be7bceb04df4cb57750dfeca007e34f82db",
      "process": "5dbdceca7188a9c6fea2e95009508463b81155be236ffc075fdd072a12c4d2ba"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | `odd lambda` = () -> new { `bad odd`: Int = 1 }\n                         ^^^\nat m2-platform#`odd lambda`.<function#1> (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "diagnostic-stage33-function-let-inferred",
    "source": "function build() = let (a = 1) new { bad: Int = a }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "e54a7372ac572890eb48295249146d273382056e24f9a9306bee324a2c9dbd87",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-let-inferred",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "cceb0c223d869cee8ea9b284e0422513a3eef0f0b18619e8144c012d3b751fa9",
      "process": "d61d1328f5ea1124d9fcbd7542359219203d9604d9d8c038722bd245e5cea394"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function build() = let (a = 1) new { bad: Int = a }\n                                              ^^^\nat m2-platform#build.<let expr>.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-function-entry-inferred",
    "source": "function build() = new { [\"bad\"] = new { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "8dfa338d20cc5069aa6760cb5e952be54654ed66cdeb3499f52eba1fe16f5c76",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-entry-inferred",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4643659f77f1f32d21f1c4d778309303ac7fe69e890b320106880da6b3a955a4",
      "process": "b8176f886d096d1b0b0e75342e66df4884013699d717dec2969e3130a3394eb0"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function build() = new { [\"bad\"] = new { bad: Int = 1 } }\n                                                  ^^^\nat m2-platform#build[\"bad\"].bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-function-body-earlier-error",
    "source": "function build() = new { before = \"\\q\"; bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "900e268c14ef8d7ce0533356b201b818b3f3f84c811b45aa80d118b1fefa4160",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-body-earlier-error",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c26f412493bbd4d8607fd10954d1ac74c8bb3e2ec7070a122b975ac7d945b716",
      "process": "259b0a64b615f2f77203bd0261c5b627a589ffcc9a9ada4405243289c17ac8ba"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | function build() = new { before = \"\\q\"; bad: Int = 1 }\n                                       ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-lambda-body-earlier-error",
    "source": "f = () -> new { before = \"\\q\"; bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "b586099cf8146de4ff3f0aa9372b41884afd0213f7b02b4cd2d8c25639376ba8",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/lambda-body-earlier-error",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3574f2a6b23f98fd7f5d3f042364a0cb690fe497f5684f3e5a3ba706afafc996",
      "process": "986f31bc4381936e4098e9b69281a552b5eb1f03ab447e9230a2a184cd8be35f"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | f = () -> new { before = \"\\q\"; bad: Int = 1 }\n                              ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-function-const-before-parent",
    "source": "function build() = new { const bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "1459d98db811d6ac4807683fabf0a9586c517c29be3383ab0e7bfc1345a3b3ae",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/function-const-before-parent",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "851a9da9a61b03a0bf1e0150516acb2aa227bd3a41e4b70a7ff677a7f0580a70",
      "process": "f56b8b321897753f794d577abf6d792902d635eb0b000150302979065d835960"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `const` can only be applied to object members that are also `local`.\n\n1 | function build() = new { const bad: Int = 1 }\n                             ^^^^^\nat m2-platform#build (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage33-let-valid-inferred",
    "source": "x = let (a = 1) new { good = a }\nresult = x.good\n",
    "expression": "result",
    "sourceSha256": "018426b2710f28b2b4ec81d725f2a1b250ae93015da96e00117092b28b544021",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/let-valid-inferred",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "520f23b7a9871c2b8e4afd6cb789a8acdffe5d424d41523fbce92db19e13b4e9"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage33-lambda-argument-new",
    "source": "f = (a) -> a\nresult = f(new { bad: Int = 1 })\n",
    "expression": "result",
    "sourceSha256": "1270ba0c1f67469bfa76790623f07972f468b8fb7bf0a7f4629cabc2e6a1c647",
    "oracleCasePath": "target/m2/stage33-oracle-01/cases/lambda-argument-new",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9e82619ee67f455f5b66a1089fca16b046f01599b8f7229f3c6ce6d7482854f1",
      "process": "c88866a3405e4cb3bcfee8ca74c877923fe9fa2f5eac2035e5816453f273d88b"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | result = f(new { bad: Int = 1 })\n               ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "diagnostic-stage33-object-local-typed-valid",
    "source": "x = new { local function build(): Dynamic = new { good = 1 }; value = build().good }\nresult = x.value\n",
    "expression": "result",
    "sourceSha256": "7301df4c09ccfbb17d4b9e84cab018e4a6445f4656f05470ed54eec6b72ca1a1",
    "oracleCasePath": "target/m2/stage33-oracle-02/cases/object-local-typed-valid",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "f1f00acefa00494ae087d37d65f1e79222dd1fcbf3e675db6e56ff585fb98d1b"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage33-function-if-let-paren-valid",
    "source": "function build(): Dynamic = if (true) (let (a = 1) (new { good = a })) else (new { good = 2 })\nresult = build().good\n",
    "expression": "result",
    "sourceSha256": "61c0f9bed486fb33b9366d49b795ded9ee1e0f48cb0c25687d957e099e615980",
    "oracleCasePath": "target/m2/stage33-oracle-02/cases/function-if-let-paren-valid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "05291b2a15644a94e2c24d57887aa31cf079aac22034a5119ba79475b55b3cbd",
      "process": "6a79021a2dcd89f5100ab578f339faf0dcfc724c72d2d032d8670412f8e81e11"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | function build(): Dynamic = if (true) (let (a = 1) (new { good = a })) else (new { good = 2 })\n                                                        ^^^\nat m2-platform#build.<let expr> (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "diagnostic-stage33-function-unused-lazy-control",
    "source": "function unused() = new { good = throw(\"unused\") }\nresult = 2\n",
    "expression": "result",
    "sourceSha256": "1f03b7713603cf05e20212cee9a58c54b7e5b53985a061b32488e9acc23c219f",
    "oracleCasePath": "target/m2/stage33-oracle-03/cases/function-unused-lazy-control",
    "captureSha256": {
      "stdout": "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "3623473c73885da504455840a48aeb4d22e27e91db0583d692192d5b890855e2"
    },
    "expectedValue": "2"
  },
  {
    "id": "diagnostic-stage33-local-const-unused-control",
    "source": "x = new { local const unused: Int = throw(\"unused\"); value = 2 }\nresult = x.value\n",
    "expression": "result",
    "sourceSha256": "8cd2cfb7652be42b9f330fe99661641b9f4f984ae0f436ce34fc2bce9b2c7ee7",
    "oracleCasePath": "target/m2/stage33-oracle-03/cases/local-const-unused-control",
    "captureSha256": {
      "stdout": "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "856aeb9f07ec7f9da74ead0d306d2673104ff86a90c8a1f7af6b44401fb0fbb9"
    },
    "expectedValue": "2"
  },
  {
    "id": "diagnostic-stage33-function-typed-valid",
    "source": "function build(): Dynamic = new { good = 1 }\nresult = build().good\n",
    "expression": "result",
    "sourceSha256": "9b080c2067e3835c59087f2abf7fa58d0862c0d813c5f38df6e125d41f0cc1cc",
    "oracleCasePath": "target/m2/stage33-oracle-03/cases/function-typed-valid",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "d061410948afccd233c16d2475145e62fb05e68bd7d0dde60e02655ddb0980e9"
    },
    "expectedValue": "1"
  }
];

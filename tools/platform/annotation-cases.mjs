// Exact pinned Java UTF-8 values and errors, without normalization.
export const context = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false,
  "oracle": {
    "version": "0.32.0",
    "artifactSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "lockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "acquisitions": [
      {
        "reportPath": "target/m2/stage35-selected-oracle-01/report.json",
        "reportSha256": "a2c74858d71dbc831dc907e5e453260039192e3bef943c0fda487569f848655e",
        "payloadPath": "target/m2/stage35-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "c1ad1e3f4327e2f085340d9133e22cdab2f146f4f9529f1731be24bc8ec14f6f",
        "cases": 79
      },
      {
        "reportPath": "target/m2/stage35-review-selected-oracle-01/report.json",
        "reportSha256": "20df17398122a7979037c8ba371a576cd2e7af7c454cc40cb53aad7b061b8ef1",
        "payloadPath": "target/m2/stage35-review-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "43888a82840f531d1756679ea7e36152f5cdf07cbac7e873ebaf85052bc91f1f",
        "cases": 6
      },
      {
        "reportPath": "target/m2/stage35-builtin-selected-oracle-01/recovered/report.json",
        "reportSha256": "feb1d63258d3415e395d8424ff65d51af7ad0c970b0295eab96b0740903bc43d",
        "payloadPath": "target/m2/stage35-builtin-selected-oracle-01/recovered/fixed-payloads.json",
        "payloadSha256": "da916ae6e814f45e198f988f5f87d55864d292ac95ed35eaf01a8a454116c019",
        "cases": 19
      },
      {
        "reportPath": "target/m2/stage35-builtin-selected-oracle-01/direct/report.json",
        "reportSha256": "b50101fe6e645ddaa6cb535856615c947801e76a555cb3b60c0fe826e7a1c5e9",
        "payloadPath": "target/m2/stage35-builtin-selected-oracle-01/direct/fixed-payloads.json",
        "payloadSha256": "efe879ef9a7efaf09f279ff30d0304a05145eab508eb65d3596b1b7a7aeaf48f",
        "cases": 1
      }
    ]
  }
};

export const cases = [
  {
    "id": "annotation-stage35-module-bare",
    "source": "@Ann\nmodule foo\nclass Ann extends Annotation { x: String }\n\nstage35Result = \"ok\"\n",
    "sourceSha256": "d05f415167def63b3d9da8ff119c2e1c4b61c82290b11ce59bc7e98fb88eb0d3",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-bare",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-module-lazy-body",
    "source": "@Ann { x = throw(\"lazy\") }\nmodule foo\nclass Ann extends Annotation { x: String }\n\nstage35Result = \"ok\"\n",
    "sourceSha256": "c7e97325e63ae19266b12cca3149af79a25d4dddd3dd33fb575ccb65469f936e",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-lazy-body",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-module-invalid",
    "source": "@Int\nmodule foo\n\nstage35Result = \"ok\"\n",
    "sourceSha256": "c12ebd968d4efe97cc6eaef021f80f6e727507e4abd6535800bc945453cb68bc",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-invalid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "889183b377f49bc1db913b1502c52e50ab583b7cc80b71977bbfd0c38bc3e2aa",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat foo (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-class-bare",
    "source": "@Ann\nclass C {}\nclass Ann extends Annotation { x: String }\n\nstage35Result = C.toString()\n",
    "sourceSha256": "63493d54b63154145140293d4a671560e01ed452dd2c639b8a0246ecc664b235",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-bare",
    "captureSha256": {
      "stdout": "27ab27267e8625ca9042aba09ab6d455d0b036826a4f5da990f0ac5019a99f21",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "m2-platform#C"
  },
  {
    "id": "annotation-stage35-class-lazy-body",
    "source": "@Ann { x = throw(\"lazy\") }\nclass C {}\nclass Ann extends Annotation { x: String }\n\nstage35Result = C.toString()\n",
    "sourceSha256": "073675466f3eb7246ba6075ff1db1d9a3fc6455642803bdfc8231164b7817e44",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-lazy-body",
    "captureSha256": {
      "stdout": "27ab27267e8625ca9042aba09ab6d455d0b036826a4f5da990f0ac5019a99f21",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "m2-platform#C"
  },
  {
    "id": "annotation-stage35-class-invalid",
    "source": "@Int\nclass C {}\n\nstage35Result = C.toString()\n",
    "sourceSha256": "8b23a43832ba634bde065a39dfb291fe26c836406485ccc00a697752c2a97f3d",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-invalid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "88b26dfeace0869e3b45fb6827e4f4d21341f263e749bb740d71285e4ea98f9c",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat m2-platform#C (file:///m2-platform.pkl)\n\n4 | stage35Result = C.toString()\n                    ^\nat m2-platform#stage35Result (file:///m2-platform.pkl)\n\n1 | stage35Result\n    ^^^^^^^^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-alias-bare",
    "source": "@Ann\ntypealias A = Int\nclass Ann extends Annotation { x: String }\n\nstage35Result = A.toString()\n",
    "sourceSha256": "1f52a7a66ea2863861a82bf23f03ae791cb5f434c0c24856e6e70ff9683e825a",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-bare",
    "captureSha256": {
      "stdout": "157dcd78e977f472f1d1ded18c0c61461d32df9f6e5ff3c17690a9e0e17c7ffb",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "m2-platform#A"
  },
  {
    "id": "annotation-stage35-alias-lazy-body",
    "source": "@Ann { x = throw(\"lazy\") }\ntypealias A = Int\nclass Ann extends Annotation { x: String }\n\nstage35Result = A.toString()\n",
    "sourceSha256": "d72140bf98c747e1a48cccfe5120c3aaa1272a6cadab435b2cb93e4793ec6e3f",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-lazy-body",
    "captureSha256": {
      "stdout": "157dcd78e977f472f1d1ded18c0c61461d32df9f6e5ff3c17690a9e0e17c7ffb",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "m2-platform#A"
  },
  {
    "id": "annotation-stage35-alias-invalid",
    "source": "@Int\ntypealias A = Int\n\nstage35Result = A.toString()\n",
    "sourceSha256": "bfb614e0af9ee868618861d61b73ce351d00ef78f80ece9a1b0b3e8a9fe788cf",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-invalid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f7da315d689e894caac9dd1d188d4727f41853f03e210035fafb8e57be6f4dcd",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat m2-platform#A (file:///m2-platform.pkl)\n\n4 | stage35Result = A.toString()\n                    ^\nat m2-platform#stage35Result (file:///m2-platform.pkl)\n\n1 | stage35Result\n    ^^^^^^^^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-property-bare",
    "source": "@Ann\nx = 1\nclass Ann extends Annotation { x: String }\n\nstage35Result = x.toString()\n",
    "sourceSha256": "1a3c3cbf999e199b50dc832473cf2e2407f23d575cff094e989112cc86e37bf4",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-bare",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "annotation-stage35-property-lazy-body",
    "source": "@Ann { x = throw(\"lazy\") }\nx = 1\nclass Ann extends Annotation { x: String }\n\nstage35Result = x.toString()\n",
    "sourceSha256": "82c35c82652eb028800833f597d1d732e2ff8bf97f71394d7a1ded544d9aad9c",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-lazy-body",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "annotation-stage35-property-invalid",
    "source": "@Int\nx = 1\n\nstage35Result = x.toString()\n",
    "sourceSha256": "0b7f7687bff878509b3a026c1e023e64b05f47b2182a24e376376a77c106655c",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-invalid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5fce5c3815cc2e059e7bb3d9a581faaf49de328de6aab314714a72f0b4bf95c3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-typed-property-bare",
    "source": "@Ann\nx: Int = 1\nclass Ann extends Annotation { x: String }\n\nstage35Result = x.toString()\n",
    "sourceSha256": "8d10a99384a9e7c85ddb5b9807445eac891fbf614e9cd4a4c6dba693aef23846",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-typed-property-bare",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "annotation-stage35-typed-property-lazy-body",
    "source": "@Ann { x = throw(\"lazy\") }\nx: Int = 1\nclass Ann extends Annotation { x: String }\n\nstage35Result = x.toString()\n",
    "sourceSha256": "6b7e42bf333b4c5ef154d3c6258e86561ea48e7746c439d7428e2ff340185015",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-typed-property-lazy-body",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "annotation-stage35-typed-property-invalid",
    "source": "@Int\nx: Int = 1\n\nstage35Result = x.toString()\n",
    "sourceSha256": "4be53a9a6aa4e798bee7f52c146f3e3e9d80c0a8bfd69b21f84baf720237a185",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-typed-property-invalid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5fce5c3815cc2e059e7bb3d9a581faaf49de328de6aab314714a72f0b4bf95c3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-method-bare",
    "source": "@Ann\nfunction f(): Int = 1\nclass Ann extends Annotation { x: String }\n\nstage35Result = f().toString()\n",
    "sourceSha256": "5e4a74fefff81603253a7d976102a3dcb5755995fbce5d00ef7b6747753d387e",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-method-bare",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "annotation-stage35-method-lazy-body",
    "source": "@Ann { x = throw(\"lazy\") }\nfunction f(): Int = 1\nclass Ann extends Annotation { x: String }\n\nstage35Result = f().toString()\n",
    "sourceSha256": "8d0b98bc10f4a0130c71795810de4fd0aa194eb1f0a7d6011761ae8778b8d48e",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-method-lazy-body",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "annotation-stage35-method-invalid",
    "source": "@Int\nfunction f(): Int = 1\n\nstage35Result = f().toString()\n",
    "sourceSha256": "02d6c11af4655f23136dd1d6a298b832f69824a9e64fe7e252573cdc8975e889",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-method-invalid",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5fce5c3815cc2e059e7bb3d9a581faaf49de328de6aab314714a72f0b4bf95c3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-property-unknown",
    "source": "@Missing\nx = 1\n\nstage35Result = x.toString()\n",
    "sourceSha256": "4ba9173a27bd581cfcd5801937e54e41101ac529706bf61f52e53c196a947352",
    "expression": "stage35Result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-unknown",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "06ccf88e6e46e569f27ed5cd7f6173983ac003ee4ff6a34971c548c319d975d4",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `Missing`.\n\n1 | @Missing\n     ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-class-unused-invalid",
    "source": "@Int\nclass C {}\nresult=\"ok\"\n",
    "sourceSha256": "060c512818b4d49c58be0a02b1e7c268efece2e4efdfda936c95b650dffa43bc",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-unused-invalid",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-alias-unused-invalid",
    "source": "@Int\ntypealias A = Int\nresult=\"ok\"\n",
    "sourceSha256": "9d3b78f33c32f3b3f4746700f4c17ebf45076a64576a3428245935e96924744d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-unused-invalid",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-property-lazy-type",
    "source": "class Ann extends Annotation { x: Int }\n@Ann { x = \"wrong\" }\nresult=\"ok\"\n",
    "sourceSha256": "d0db53513dc093c5de0b8952c630b37505b237c059090833c8ba3318b21d3320",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-lazy-type",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-property-lazy-unknown",
    "source": "class Ann extends Annotation {}\n@Ann { nope = throw(\"lazy\") }\nresult=\"ok\"\n",
    "sourceSha256": "ff2bb837b22b2dffb0f492575d0dd243dd5f18eb241965625f07da8afd1ff0e6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-lazy-unknown",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "09ee0eb588b6baa2db10efda622d4d711eae0f13effc568d9c3be334c34e88c8",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find property `nope` in object of type `m2-platform#Ann`.\n\n2 | @Ann { nope = throw(\"lazy\") }\n           ^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nAvailable properties:\n(none)\n"
  },
  {
    "id": "annotation-stage35-property-missing-required",
    "source": "class Ann extends Annotation { x: Int }\n@Ann\nresult=\"ok\"\n",
    "sourceSha256": "cb29844f6f8431cd709ee05ff1020514d6fe205a91ffa1db8e2c2f4c4a129c08",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-missing-required",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-annotation-base",
    "source": "@Annotation\nresult=\"ok\"\n",
    "sourceSha256": "87283b72a236ec5cd736b4a5ed90cb96f274681b3ed7e3eb131e7da0bab596e8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-annotation-base",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ad1c131d8f48116d674aee38d48c5bfcdbf2837eec76215d6af65b8141bc0bf0",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot instantiate abstract class `Annotation`.\n\n1 | @Annotation\n     ^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nAbstract classes cannot be instantiated.\nInstead, instantiate a concrete subclass.\n"
  },
  {
    "id": "annotation-stage35-custom-derived",
    "source": "open class Ann extends Annotation\nclass More extends Ann\n@More\nresult=\"ok\"\n",
    "sourceSha256": "98bee0c6786d53df655be7c8a3538bc9094b0f81f0b34024e867871c81b81ee4",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-custom-derived",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-alias-class",
    "source": "typealias A = Ann\nclass Ann extends Annotation\n@A\nresult=\"ok\"\n",
    "sourceSha256": "82c1bdcb8c21471e67dcf097abc55ec63e54222e0fda4b6341a805ff8b83c6b9",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-class",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-alias-union",
    "source": "typealias A = Ann|Int\nclass Ann extends Annotation\n@A\nresult=\"ok\"\n",
    "sourceSha256": "84c8abee366a93c6bd25f921b89715e09aff4738c91c042e6e814ced5274e73a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-union",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f94124d2909e57a18b1b889092292679b87f9634105192ffd03ab18c58b68bf1",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n3 | @A\n     ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-type-union",
    "source": "class Ann extends Annotation\n@Ann|Int\nresult=\"ok\"\n",
    "sourceSha256": "35479fc11413d159c6bd03c1ee9e40083838a54f566a364423b9a930a559b2a8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-type-union",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6c04303289f08a80d7e24098a58f77c0470d3d9223a8200b280eb932f456e2a9",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n2 | @Ann|Int\n     ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-type-nullable",
    "source": "class Ann extends Annotation\n@Ann?\nresult=\"ok\"\n",
    "sourceSha256": "6448d8338dbb8c63614dece4263ef7408f6da7b89c3d9b7e0b50c7d0c36833aa",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-type-nullable",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e84b02a6924d67c5745db889bcab15006289dd784636d0739fb0add5270dd45a",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n2 | @Ann?\n     ^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-type-parenthesized",
    "source": "class Ann extends Annotation\n@(Ann)\nresult=\"ok\"\n",
    "sourceSha256": "a65c2618248cdb4549bd78fe5f49a2a28d474c2f2009da48264a8afd132bfa3b",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-type-parenthesized",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-type-constrained",
    "source": "class Ann extends Annotation\n@Ann(true)\nresult=\"ok\"\n",
    "sourceSha256": "910fc50bc13803022bce5c10263d2454ba52fea8728f3778fc42ade1d4fcabdc",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-type-constrained",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7299aa7ec55947149c9aa52ed003d5a8d636ed725b4caa76231c8b07169d6bb7",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n2 | @Ann(true)\n     ^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-type-value",
    "source": "const ann = Annotation\n@ann\nresult=\"ok\"\n",
    "sourceSha256": "68c67e5f94ab210ae5fcfc20e2e36b933371e7fa847f53c21f9f6318a7e93664",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-type-value",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5cdc13a87ef234820709719b7da1745d2b26ba7545691f2e32e7d4283d258121",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected `ann` to be a type, but it is not.\n\n2 | @ann\n     ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-annotation-order",
    "source": "@MissingOne\n@MissingTwo\nresult=\"ok\"\n",
    "sourceSha256": "963ab5e9072a19c5f713a996aed5fb4d5dbe92aab0186add27a12a40bd8669fd",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-annotation-order",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9c117cc1f917e75e777e804ad8b8f37183feab68a054897a4cda08a4d7f64c45",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingOne`.\n\n1 | @MissingOne\n     ^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-class-super-before-annotation",
    "source": "@MissingAnn\nclass C extends MissingSuper {}\nresult=C.toString()\n",
    "sourceSha256": "1d898a60cb06219e59a1d42f415991bedd016b47c5bde0bd64d687b08727cfaa",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-super-before-annotation",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c43fdb08f113c044beb54f6ec9db7ce81c1e01b00b07e22f00a59d750b9fa0fd",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingSuper`.\n\n2 | class C extends MissingSuper {}\n                    ^^^^^^^^^^^^\nat m2-platform#C (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n3 | result=C.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-class-annotation-before-property",
    "source": "@MissingAnn\nclass C { x: MissingType }\nresult=C.toString()\n",
    "sourceSha256": "5bef31a882296f14d9a614a89b8e7dc6ea0e6a7ebf96c2ab6f14fcbdecf2ae73",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-annotation-before-property",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "815608041e4fc15545dc8fa06a2033c319fb925f8115d531ac53c57886d2e26d",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingAnn`.\n\n1 | @MissingAnn\n     ^^^^^^^^^^\nat m2-platform#C (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n3 | result=C.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-alias-annotation-before-rhs",
    "source": "@MissingAnn\ntypealias A = MissingType\nresult=A.toString()\n",
    "sourceSha256": "42976a65691e004782a02e93b26857daf5c2222780a04eb2b7fa74490d3216b6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-annotation-before-rhs",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "cc39b9a565f74e08b5a3988640b815a2a296c7a11ab280df23a37fa7e20b7e4e",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingAnn`.\n\n1 | @MissingAnn\n     ^^^^^^^^^^\nat m2-platform#A (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n3 | result=A.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-property-annotation-before-type",
    "source": "@MissingAnn\nresult: MissingType = \"ok\"\n",
    "sourceSha256": "378972fcdc7aa72af97eb9a83d12d671ac59271283f439dbaae96f9d651cd935",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-annotation-before-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c72e0b297866e451f5f948232b8781222db891b818c4242c49835116e119b005",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingAnn`.\n\n1 | @MissingAnn\n     ^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-method-annotation-before-parameters",
    "source": "@MissingAnn\nfunction f(x: MissingType): MissingResult = x\nresult=\"ok\"\n",
    "sourceSha256": "6c2abc5c5b029c8759c1782532a83d206e2d54db2987df223014e36aacdd90be",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-method-annotation-before-parameters",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c72e0b297866e451f5f948232b8781222db891b818c4242c49835116e119b005",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingAnn`.\n\n1 | @MissingAnn\n     ^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-property-before-method",
    "source": "@MissingMethod\nfunction f()=1\n@MissingProperty\nresult=\"ok\"\n",
    "sourceSha256": "24b67de0ae74a9a349604056c151be5e07b4fb3c16d850bf4397cfb215c5cc9f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-property-before-method",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "fcfd667d563fffe722d03b041ee2c963ad9c0fff20885f21ac4f1c81092f0109",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `MissingProperty`.\n\n3 | @MissingProperty\n     ^^^^^^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-class-local-property-unused",
    "source": "class C { @Int local x = 1 }\nresult=C.toString()\n",
    "sourceSha256": "9871cb03625df2dfe7a469646c62302cf67dfe7ec8ea3eb021277ad892275a09",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-local-property-unused",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4ba6bb1bcad1463b73faef52ec59465c33cda19e9fe7e89aa2c0bf1ff0a1a1c8",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | class C { @Int local x = 1 }\n               ^^^\nat m2-platform#C (file:///m2-platform.pkl)\n\n2 | result=C.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-class-local-method-unused",
    "source": "class C { @Int local function f()=1 }\nresult=C.toString()\n",
    "sourceSha256": "14e1e57f2be0d2f6987481b8982d72ea9a245c5e4bf6751bd7eeba3e224efb7d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-local-method-unused",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "eed8ea66fc146771ad58e6cd309dfcbf4b67587ad9f89d43c6453513f2a4857e",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | class C { @Int local function f()=1 }\n               ^^^\nat m2-platform#C (file:///m2-platform.pkl)\n\n2 | result=C.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-module-local-property-unused",
    "source": "@Int local x = 1\nresult=\"ok\"\n",
    "sourceSha256": "af08f6f2871f07b3ad3ee4d4fb4a00248b557b37fc9a5780076f9f90e95a7bd1",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-local-property-unused",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a9a5bee4570d6dcfc5269f3636a24d36688e9e30bbcca1a21cd428becbdf2282",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int local x = 1\n     ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-module-local-method-unused",
    "source": "@Int local function f()=1\nresult=\"ok\"\n",
    "sourceSha256": "51f7a12379270e1853a64561adda3ce3656c03e5926d4856b02423893adf9f76",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-local-method-unused",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d5268ab52e6c01847736818b60000edddee324f32cca4d8999494d42c3b118a6",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected an annotation class.\n\n1 | @Int local function f()=1\n     ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-module-local-class-unused",
    "source": "@Int local class C {}\nresult=\"ok\"\n",
    "sourceSha256": "b40674adb5e5de2cfbd7ce180bf7dbc3f9f281395310011c4e8505b8a11a2111",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-local-class-unused",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-module-local-alias-unused",
    "source": "@Int local typealias A = Int\nresult=\"ok\"\n",
    "sourceSha256": "339aac352d1a62df1f8c691911221a64ee6c5a5fcc31b861bf80ce6d8652980f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-local-alias-unused",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-object-property",
    "source": "class Ann extends Annotation\nresult=(new Dynamic { @Ann x=1 }).x.toString()\n",
    "sourceSha256": "b4e4a7fc0a8b0726cbebcb39e22d77af0f7e59c7d5ae5f9fa14563f56803f644",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-object-property",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ce2c2319026881e647b46a87b44d693115da9a1048b6b4414fd6aaba93215ee3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected token `@`. Expected `}`.\n\n2 | result=(new Dynamic { @Ann x=1 }).x.toString()\n                          ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-deprecated-property-message",
    "source": "@Deprecated { message = throw(\"lazy\") }\nresult=\"ok\"\n",
    "sourceSha256": "1e99a74a50000dd2b934aadc1f186c194c657be0141c74bc1a1396f7044bfa8a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-property-message",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-class-message",
    "source": "@Deprecated { message = throw(\"lazy\") }\nclass C {}\nresult=C.toString()\n",
    "sourceSha256": "05b4caa3d43054fdf1fddc0cdfe13b8f45626ed92711afb13a4a877f473ed6e3",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-class-message",
    "captureSha256": {
      "stdout": "27ab27267e8625ca9042aba09ab6d455d0b036826a4f5da990f0ac5019a99f21",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "m2-platform#C"
  },
  {
    "id": "annotation-stage35-alias-generic",
    "source": "class Ann extends Annotation\ntypealias A<T> = Ann\n@A<Int>\nresult=\"ok\"\n",
    "sourceSha256": "8807f60daf988e2b88b06a5fd6953ebc7b2cf40383c22309c1c9143eebcbbd0d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-generic",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-alias-generic-missing",
    "source": "class Ann extends Annotation\ntypealias A<T> = Ann\n@A\nresult=\"ok\"\n",
    "sourceSha256": "2d13aa05295002dc6c166e341c3f1a4c68c1fd13f1c4c65d9177650f2d8ee239",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-generic-missing",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-alias-generic-arity",
    "source": "class Ann extends Annotation\ntypealias A<T> = Ann\n@A<Int,String>\nresult=\"ok\"\n",
    "sourceSha256": "88662b03a65fadd671104880b8726f0d58cedd710b3f6e819a35d3c943e8c849",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-generic-arity",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ee9d04540d352fcdfdf10327d1fef1091203410ee3575b08fc872f282269f724",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected 1 type argument(s) but got 2.\n\n3 | @A<Int,String>\n     ^^^^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-alias-generic-unknown-arg",
    "source": "class Ann extends Annotation\ntypealias A<T> = Ann\n@A<Missing>\nresult=\"ok\"\n",
    "sourceSha256": "14618e4b1d3e9546b9393c4fdaf186bc21a9439e9d16df03bed35f580d943300",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-alias-generic-unknown-arg",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "24e2aa433170274a907b4ec0f69c17abd3f96400bf802ea02746aee5e3b659b0",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `Missing`.\n\n3 | @A<Missing>\n       ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-class-unexpected-type-args",
    "source": "class Ann extends Annotation\n@Ann<Int>\nresult=\"ok\"\n",
    "sourceSha256": "c10ff3bd778572c1d5f7aa9300012989e2bd0e26dca27d9290230943814ac673",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-class-unexpected-type-args",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "96b52ed594f2cd46c60443c39a0bb29eac66efd2eca23c985ebade756d1de1c3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected 0 type argument(s) but got 1.\n\n2 | @Ann<Int>\n     ^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-body-generator-before-next",
    "source": "class Ann extends Annotation\n@Ann { when (throw(\"first\")) {} }\n@Missing\nresult=\"ok\"\n",
    "sourceSha256": "559dbb71c6f189d7c3e84f395316c88c33daba66098f132916431de6b8c953c8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-generator-before-next",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c8fb8c987ca8834789e78f70b9b02e4c9fa86210bd69c96344a0b82d3be929c9",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nfirst\n\n2 | @Ann { when (throw(\"first\")) {} }\n                 ^^^^^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-body-spread-before-next",
    "source": "class Ann extends Annotation\n@Ann { ...throw(\"spread\") }\n@Missing\nresult=\"ok\"\n",
    "sourceSha256": "14e6aa6a67062449d47fe72af6a95838953216ad4121dfbb09a6caf5c27e9bb6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-spread-before-next",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7d4938d38016210acfe926bdddd73f1b0cfc7eee21b40571729b97481cec1e35",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nspread\n\n2 | @Ann { ...throw(\"spread\") }\n              ^^^^^^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-body-eager-for",
    "source": "class Ann extends Annotation\n@Ann { for (x in throw(\"for\")) {} }\nresult=\"ok\"\n",
    "sourceSha256": "f25debbf490ba215dc1d10bb04b67fefe5b5c5147485cd11f0a58f00fcb180cb",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-eager-for",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "36366682a16710f3e613819f3cf7ef68ff4cafb3ef9081231888f1845fdbf0a3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nfor\n\n2 | @Ann { for (x in throw(\"for\")) {} }\n                     ^^^^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-body-eager-unknown-lazy-member",
    "source": "class Ann extends Annotation { x: Int }\n@Ann { when (false) { unknown=1 } }\nresult=\"ok\"\n",
    "sourceSha256": "ad6946003d99744d31bfb65f3a21fc6b1e85fd20193aa7688a7b2662a90545ca",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-eager-unknown-lazy-member",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "031667884fa660566f09a5e5a4097c4149e397cd40e6d1f4200bb81dece07086",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected token `unknown`. Expected `}`.\n\n2 | @Ann { when (false) { unknown=1 } }\n                          ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-body-missing-eager-field",
    "source": "class Ann extends Annotation { x: Int }\n@Ann { when (this.x==1) {} }\nresult=\"ok\"\n",
    "sourceSha256": "31d4f19b2caa8aefdc832b053850eb537932a92cff5fe3df69472e15f3d11b6a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-missing-eager-field",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "126eb9b3248da1d707ac35055f2581542cfbc6b51d50db1895e37efbb8f83f07",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find property `x` in module `m2-platform`.\n\n2 | @Ann { when (this.x==1) {} }\n                 ^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nAvailable properties in module `m2-platform`:\nAnn\noutput\n"
  },
  {
    "id": "annotation-stage35-body-nonconst-eager",
    "source": "class Ann extends Annotation\nflag=true\n@Ann { when (flag) {} }\nresult=\"ok\"\n",
    "sourceSha256": "02ca8fabb5f99c61e2c6c5ebe2b7034962ad5ecdeee8d406882de735ed218b70",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-nonconst-eager",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "24fcc06b7eea81ff3e129d26e69423619bf035f3e2d5c695c22360147065d02b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot reference property `flag` from here because it is not `const`.\n\n3 | @Ann { when (flag) {} }\n                 ^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to property `flag`\n2. Self-import this module, and reference this property from the import.\n"
  },
  {
    "id": "annotation-stage35-body-const-eager",
    "source": "class Ann extends Annotation\nconst flag=true\n@Ann { when (flag) {} }\nresult=\"ok\"\n",
    "sourceSha256": "89765f957bcea6f32aad47494d1c15e343080970f48088275d30a283cd1c43c1",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-body-const-eager",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-body-nonconst",
    "source": "value=\"x\"\n@Deprecated { message=value }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "196901129a3b5cb7caf355ceef5cab43b841e277fe74b108814e10d42ca306dc",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-nonconst",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "eb05f1231bbe549d276b797b50680d19f436c8e8911af806164e76b5fb1e1bb2",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot reference property `value` from here because it is not `const`.\n\n2 | @Deprecated { message=value }\n                          ^^^^^\nat m2-platform#f.message (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to property `value`\n2. Self-import this module, and reference this property from the import.\n"
  },
  {
    "id": "annotation-stage35-deprecated-body-const",
    "source": "const value=\"x\"\n@Deprecated { message=value }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "d5e08cf0c833898f8ceb28fec9982f2ee5b811ee4121b4c259706eadc440c0a2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-const",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-body-localconst",
    "source": "local const value=\"x\"\n@Deprecated { message=value }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "cde136746aba05489406190bac2d8884ecdcfa5c6bb7073b2ee20088db381b8d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-localconst",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-body-qualified-module",
    "source": "value=\"x\"\n@Deprecated { message=module.value }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "66bf5855442fd472cfe2a10f6c29cb4c5724fa828312bc0b5f8790de076071e8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-qualified-module",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c60d173cc416f029aa169935f0533ef342abf42abc4d75290f5fac3bdea27a53",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot reference property `value` from here because it is not `const`.\n\n2 | @Deprecated { message=module.value }\n                          ^^^^^^^^^^^^\nat m2-platform#f.message (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to property `value`\n2. Self-import this module, and reference this property from the import.\n"
  },
  {
    "id": "annotation-stage35-deprecated-body-qualified-outer",
    "source": "value=\"x\"\n@Deprecated { message=outer.value }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "71a39b594ea3c7a6c740398dc6425f5d9cc4da1f2173f756e7c692d5e4cdf7c7",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-qualified-outer",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "559e152b93365ac5fb7c6c1d9a85700f6cf2342182989d1249fda93ff991143a",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot reference property `value` from here because it is not `const`.\n\n2 | @Deprecated { message=outer.value }\n                          ^^^^^^^^^^^\nat m2-platform#f.message (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to property `value`\n2. Self-import this module, and reference this property from the import.\n"
  },
  {
    "id": "annotation-stage35-deprecated-body-lambda",
    "source": "value=\"x\"\n@Deprecated { message=((x)->value).apply(1) }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "db1b3ebf39ff364bd5130b7e06aba7bde0366f7389bcc3ae262dfb9ab23f5cdb",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-lambda",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "cc87a0947c55ef222bda189837e7e96d0480fccb5e27edcee8e638b98fb310fc",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot reference property `value` from here because it is not `const`.\n\n2 | @Deprecated { message=((x)->value).apply(1) }\n                                ^^^^^\nat m2-platform#f.message.<function#1> (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to property `value`\n2. Self-import this module, and reference this property from the import.\n\n2 | @Deprecated { message=((x)->value).apply(1) }\n                          ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#f.message (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-deprecated-body-function-const",
    "source": "const function message()=\"x\"\n@Deprecated { message=message() }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "603876b73694ba01a9deb2f0c3a8c262a38d55dda13254aebde9059f7c9de921",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-function-const",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-body-function-nonconst",
    "source": "function msg()=\"x\"\n@Deprecated { message=msg() }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "0e3fdb529b12249ce816618a23f4ce532aa5af9e76a23da8c9552d6abb0a071b",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-function-nonconst",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5ba475ca2a9d835b625e54d8a2a7de6017be12cd9310ecaa6c52ec2200f2eba8",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot call method `msg` from here because it is not `const`.\n\n2 | @Deprecated { message=msg() }\n                          ^^^^^\nat m2-platform#f.message (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to method `msg`\n2. Self-import this module, and reference this method from the import.\n"
  },
  {
    "id": "annotation-stage35-deprecated-body-class-member",
    "source": "class C { value=\"x\"; @Deprecated { message=value } function f()=1 }\nresult=C.toString()\n",
    "sourceSha256": "6852777d840b553af6bad66944a425dccbe7f234514000b6c71b55c2b2369ae0",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-class-member",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1bc031e2dd78bcbe06c2091f4e2f7a2dde8bc10de085ced87dde4f9840787b5b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find property `value`.\n\n1 | class C { value=\"x\"; @Deprecated { message=value } function f()=1 }\n                                               ^^^^^\nat m2-platform#C.f.message (file:///m2-platform.pkl)\n\n2 | result=C.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-deprecated-body-this",
    "source": "@Deprecated { message=this.toString() }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "e2e1be81a5e6e49ba78e1c99404cb68cb1eec77e0778ad1994c813a0c4ec28ff",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-this",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-body-local-member",
    "source": "@Deprecated { local text=\"ok\"; message=text }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "9186720c5ac9e477b5f1b96620e9e270e79cb55a0fcfc6e356e79b5cb392414c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-body-local-member",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-message-wrong-type",
    "source": "@Deprecated { message=1 }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "09c3de78fb59ad81ea1c1f4e8fcf2bb0c77295a4451bc085f5b137400a9a7213",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-message-wrong-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "31b15118e22085607cd7b0ad653545f2e20cca6d3925d6fa84b9c4c9b1727b14",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n159 | message: String?\n               ^^^^^^\nat pkl.base#Deprecated.message (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L159)\n\n1 | @Deprecated { message=1 }\n                          ^\nat m2-platform#f.message (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-deprecated-method-first-message",
    "source": "@Deprecated { message=\"first\" }\n@Deprecated { message=throw(\"second\") }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "d8963242996b2971134f7832ab62aebd4a8045aa3ebb330fec8ea66a710c6a3d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-method-first-message",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-deprecated-method-after-signature",
    "source": "@Deprecated { message=throw(\"message\") }\nfunction f(x: Missing)=1\nresult=\"ok\"\n",
    "sourceSha256": "723c052fa7483e4e771d8602e7e53cac9800c7eac753feb9f08468f448be952e",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-deprecated-method-after-signature",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6b60146535cf0c37935c6d631513c4bf16e4e62871612208db6716ff708304eb",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `Missing`.\n\n2 | function f(x: Missing)=1\n                  ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "annotation-stage35-annotation-body-duplicate",
    "source": "class Ann extends Annotation { x: Int }\n@Ann { x=1; x=2 }\nresult=\"ok\"\n",
    "sourceSha256": "573edbfe3324d24899bbc552ebd43c8f441b148c456a0cfa5d8bc3edb3154402",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-annotation-body-duplicate",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "82df00e4fae8d520893781410e7d69db6647168028be5fbd332f7b3120fed614",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nDuplicate definition of member `x`.\n\n2 | @Ann { x=1; x=2 }\n                ^\nat m2-platform#result (file:///m2-platform.pkl)\n"
  },
  {
    "id": "annotation-stage35-annotation-body-parameter",
    "source": "class Ann extends Annotation\n@Ann { x -> }\nresult=\"ok\"\n",
    "sourceSha256": "72310677057d9509988cc318b39cc2cd9747486a7a547bc0bbbb93de33511908",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-annotation-body-parameter",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-module-body-const",
    "source": "@Deprecated { message=constValue }\nmodule foo\nconst constValue=\"ok\"\nresult=\"ok\"\n",
    "sourceSha256": "c16521afb25cbc6e5e80a56c7d16a6ae7643fbe4cbada3ffdf66a9edfe25260e",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-module-body-const",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "annotation-stage35-typealias-body-order",
    "source": "class Ann extends Annotation { x: Int }\n@Ann { when (throw(\"annotation\")) {} }\ntypealias A = Missing\nresult=A.toString()\n",
    "sourceSha256": "0182285126ac162e5e23d4992e442f7a62c622e1f91883546ba1e378f0952718",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-selected-oracle-01/cases/annotation-stage35-typealias-body-order",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9e9c79dff3545a4abca5770680d132a2fb4043126ce3153783979ee3ae51d2c2",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nannotation\n\n2 | @Ann { when (throw(\"annotation\")) {} }\n                 ^^^^^^^^^^^^^^^^^^^\nat m2-platform#A (file:///m2-platform.pkl)\n\n4 | result=A.toString()\n           ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-review-deprecation-sync-next-signature",
    "source": "class Child {\n @Deprecated { ...(new Dynamic { message = \"ok\" }) }\n function good() = 1\n function bad(x: Missing) = 2\n}\nresult = Child == Child\n",
    "sourceSha256": "0e44856fe88ffcd2282447403a7234432fb23628852cb03a1783f37d2a35f13c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-review-selected-oracle-01/cases/annotation-stage35-review-deprecation-sync-next-signature",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f9f4946829c74fb563c01cd2cac19a911f2b8ab1db3fd98688a5de51d88b2915",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `Missing`.\n\n4 | function bad(x: Missing) = 2\n                    ^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n6 | result = Child == Child\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-review-deprecation-literal-next-signature-control",
    "source": "class Child {\n @Deprecated { message = \"ok\" }\n function good() = 1\n function bad(x: Missing) = 2\n}\nresult = Child == Child\n",
    "sourceSha256": "7b715a7833622ffc9cf53fb123648eb098d493964486c4f55ad1d77ebf80aa87",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-review-selected-oracle-01/cases/annotation-stage35-review-deprecation-literal-next-signature-control",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f9f4946829c74fb563c01cd2cac19a911f2b8ab1db3fd98688a5de51d88b2915",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot find type `Missing`.\n\n4 | function bad(x: Missing) = 2\n                    ^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n6 | result = Child == Child\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-review-deprecated-typed-spread",
    "source": "class Child {\n @Deprecated { ...(new Deprecated { message = \"ok\" }) }\n function good() = 1\n}\nresult = Child == Child\n",
    "sourceSha256": "feb1ca7c8982c3b843970a9a6d83ddee470c9158980c3edc7b3a77f1fd22850f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-review-selected-oracle-01/cases/annotation-stage35-review-deprecated-typed-spread",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1a27123b5d75fb668dd73fd7d54116367a557b1272cb29798bc46bf539064b76",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot iterate over value of type `Deprecated`.\nValue: new Deprecated { since = ?; message = ?; replaceWith = ? }\n\n2 | @Deprecated { ...(new Deprecated { message = \"ok\" }) }\n                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\n`Typed` values are not iterable. If you mean to spread its members, convert it to `Dynamic` using `toDynamic()`.\n\n5 | result = Child == Child\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-review-deprecated-const-spread",
    "source": "const ann = new Deprecated { message = \"ok\" }\nclass Child {\n @Deprecated { ...(ann) }\n function good() = 1\n}\nresult = Child == Child\n",
    "sourceSha256": "bb0e3cac72b76e3d62a539e4a2b2665179ef4648e0f197c4d976656c6c1164ef",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-review-selected-oracle-01/cases/annotation-stage35-review-deprecated-const-spread",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9a4ea6453673a05a081958407ef1f3a29498af2ebfd75ac8b96a1fff242127ce",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot iterate over value of type `Deprecated`.\nValue: new Deprecated { since = ?; message = ?; replaceWith = ? }\n\n3 | @Deprecated { ...(ann) }\n                      ^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\n`Typed` values are not iterable. If you mean to spread its members, convert it to `Dynamic` using `toDynamic()`.\n\n6 | result = Child == Child\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-review-member-eager-const-when",
    "source": "const ann = \"ok\"\nclass Child {\n @Deprecated { when (ann == \"ok\") { message = \"ok\" } }\n function good() = 1\n}\nresult = Child == Child\n",
    "sourceSha256": "fecfe0911e0e9561cb4bb63a0c3d172e4e6a72db8ed5e0c4631e59fb8989ec13",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-review-selected-oracle-01/cases/annotation-stage35-review-member-eager-const-when",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "annotation-stage35-review-member-lazy-const-control",
    "source": "const ann = \"ok\"\nclass Child {\n @Deprecated { message = ann }\n function good() = 1\n}\nresult = Child == Child\n",
    "sourceSha256": "683a07c24109fb0a21f100bc4a1781a89de26a9cce57a9465ab3ee3a4d1d770c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-review-selected-oracle-01/cases/annotation-stage35-review-member-lazy-const-control",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "annotation-stage35-builtin-annotation-abstract",
    "source": "result=(new Annotation {}).getClass().toString()\n",
    "sourceSha256": "58cc6030797c2cf37f040777128f8ee521643ee35ddc10cfac9bf1b80d5098f4",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-annotation-abstract",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c5b500d4b38d957eb6331362e7ba4ead476134fc9994daf7c06dc6e158edd710",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot instantiate abstract class `Annotation`.\n\n1 | result=(new Annotation {}).getClass().toString()\n                ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\nAbstract classes cannot be instantiated.\nInstead, instantiate a concrete subclass.\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-deprecated-nullable-defaults",
    "source": "empty=new Deprecated {}\nfull=new Deprecated { since=\"0.1\"; message=\"Use x.\"; replaceWith=\"x\" }\nresult=List(empty.since,empty.message,empty.replaceWith,full.since,full.message,full.replaceWith,(full is Annotation)).toString()\n",
    "sourceSha256": "41682684875057e96bf5cbc063b3a1227c4616a2a90880398acad9526a51bc1e",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-deprecated-nullable-defaults",
    "captureSha256": {
      "stdout": "f7b285410dfc50902186700cb272a27144087aeb7a2ca0763b486fbae469527b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(null, null, null, \"0.1\", \"Use x.\", \"x\", true)"
  },
  {
    "id": "annotation-stage35-builtin-deprecated-since-wrong-type",
    "source": "ann=new Deprecated { since=1 }\nresult=ann.since.toString()\n",
    "sourceSha256": "f77e8596dab8c7d1298fc85cfb2619f1be53d1b94cd8220c27d194cdb0d5f393",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-deprecated-since-wrong-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a5045c085e9ec7f7b0f3633366362d7fb6b83b69d9d741f43890f2463d8a95d7",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n151 | since: String?\n             ^^^^^^\nat pkl.base#Deprecated.since (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L151)\n\n1 | ann=new Deprecated { since=1 }\n                               ^\nat m2-platform#ann.since (file:///m2-platform.pkl)\n\n2 | result=ann.since.toString()\n           ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-deprecated-message-wrong-type",
    "source": "ann=new Deprecated { message=1 }\nresult=ann.message.toString()\n",
    "sourceSha256": "787eef8cdc1351275f3973934f85c067c84e118bd0b53c0b27dfac7f64e62e9c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-deprecated-message-wrong-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7f868a92bd53a9709cdfaa94f8292b5996478f60092f81f9995ccabcfe08f4aa",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n159 | message: String?\n               ^^^^^^\nat pkl.base#Deprecated.message (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L159)\n\n1 | ann=new Deprecated { message=1 }\n                                 ^\nat m2-platform#ann.message (file:///m2-platform.pkl)\n\n2 | result=ann.message.toString()\n           ^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-deprecated-replace-with-wrong-type",
    "source": "ann=new Deprecated { replaceWith=1 }\nresult=ann.replaceWith.toString()\n",
    "sourceSha256": "887e8f764a87487c457639773682d1ed11d9221b440b7b85ef19505299bf37d8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-deprecated-replace-with-wrong-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b515b52c2336b4bef3ca80c26d2eb749cf62c5a08b4c6e3291a460e1e3cf9e03",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n182 | replaceWith: String?\n                   ^^^^^^\nat pkl.base#Deprecated.replaceWith (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L182)\n\n1 | ann=new Deprecated { replaceWith=1 }\n                                     ^\nat m2-platform#ann.replaceWith (file:///m2-platform.pkl)\n\n2 | result=ann.replaceWith.toString()\n           ^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-since-version-undefined",
    "source": "ann=new Since {}\nresult=ann.version\n",
    "sourceSha256": "84e3b70428b1b1b29d32e1f1f80db85b0d439e0d65c9e6ed313914ba7f0cbd72",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-since-version-undefined",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a15e6218dff737ad446ef932f1a830c8b4d224e58e2f7ba1fbfa792c3cd2b205",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nTried to read property `version` but its value is undefined.\n\n145 | version: String\n      ^^^^^^^\nat pkl.base#Since.version (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L145)\n\n2 | result=ann.version\n           ^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-since-version-value",
    "source": "ann=new Since { version=\"0.32.0\" }\nresult=ann.version+\"|\"+(ann is Annotation).toString()\n",
    "sourceSha256": "ffdc11bf9e3971f2dfb141efa45c4710eaa04f7335e0092488ab63580ebf09f4",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-since-version-value",
    "captureSha256": {
      "stdout": "a6cd248b8a1fced38f689fb9a90851ec5ef4944904fab7083cca167b58b44659",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "0.32.0|true"
  },
  {
    "id": "annotation-stage35-builtin-unlisted-empty",
    "source": "@Unlisted\nclass C {}\nann=new Unlisted {}\nresult=(ann is Annotation).toString()\n",
    "sourceSha256": "b29f7cf40897ec343c47d7aaaac6bdc900f81e4162adecd168385fa68a895df5",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-unlisted-empty",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "annotation-stage35-builtin-unlisted-final",
    "source": "class Child extends Unlisted {}\nresult=Child.toString()\n",
    "sourceSha256": "9255f2f7615a9866e1c7185abf08b967e2a3bfa84d2048203c527bf3bda40f39",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-unlisted-final",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6099027749eaab26fcc4221737e94d22e572015f84f061ff1a6d47f6e260d5c3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nCannot extend non-open class `Unlisted`.\n\n1 | class Child extends Unlisted {}\n                        ^^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nBy default, classes are closed for extension.\nTo make a class extensible, add an `open` modifier: `open class MyClass { ... }`\n\n2 | result=Child.toString()\n           ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-also-known-as-default-and-lazy",
    "source": "empty=new AlsoKnownAs {}\ninvalid=new AlsoKnownAs { names { 1 } }\nresult=List(empty.names.length,invalid.names.length,(invalid is Annotation)).toString()\n",
    "sourceSha256": "026a636ae653fdcf5b12a976ae6349d9e36e91e34f121696078c225ffc2a2202",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-also-known-as-default-and-lazy",
    "captureSha256": {
      "stdout": "b9237fdc9109b7ac62a8bf6f4a717d4ba7b515c67c3812ed0acdea2590c1fb2d",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(0, 1, true)"
  },
  {
    "id": "annotation-stage35-builtin-also-known-as-invalid-element",
    "source": "ann=new AlsoKnownAs { names { 1 } }\nresult=ann.names[0].toString()\n",
    "sourceSha256": "8727954cceea6d4248717ca55da630c2109a7399a7c858b286c44aedf140d647",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-also-known-as-invalid-element",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "21190c644e61703dc9e8d3ec9049a20fc30bf2fb00c765a995fd96e60c60ba94",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n190 | names: Listing<String>\n                     ^^^^^^\nat pkl.base#AlsoKnownAs.names (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L190)\n\n1 | ann=new AlsoKnownAs { names { 1 } }\n                                  ^\nat m2-platform#ann.names[#1] (file:///m2-platform.pkl)\n\n2 | result=ann.names[0].toString()\n           ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-doc-example-default-and-lazy",
    "source": "empty=new DocExample {}\ninvalid=new DocExample { subjects { 1 } }\nresult=List(empty.subjects.length,invalid.subjects.length,(invalid is Annotation)).toString()\n",
    "sourceSha256": "f421c6c763473b569ecef87c9eb18797393cec02bafa2c23ae72516b883e2025",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-doc-example-default-and-lazy",
    "captureSha256": {
      "stdout": "b9237fdc9109b7ac62a8bf6f4a717d4ba7b515c67c3812ed0acdea2590c1fb2d",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(0, 1, true)"
  },
  {
    "id": "annotation-stage35-builtin-doc-example-invalid-element",
    "source": "ann=new DocExample { subjects { 1 } }\nresult=ann.subjects[0].toString()\n",
    "sourceSha256": "8e8176ac5c33479aca2bf8f94ed045ac5d3eef50719ba189d92065ad571baed8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-doc-example-invalid-element",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8ebf2c4cb5e10412720c8f92358650ac75fd31efcc1a58bfb6e53cd35ecfc4ac",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n199 | subjects: Listing<String>\n                        ^^^^^^\nat pkl.base#DocExample.subjects (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L199)\n\n1 | ann=new DocExample { subjects { 1 } }\n                                    ^\nat m2-platform#ann.subjects[#1] (file:///m2-platform.pkl)\n\n2 | result=ann.subjects[0].toString()\n           ^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-source-code-defaults",
    "source": "ann=new SourceCode {}\nresult=List(ann.language,ann.prefix,ann.suffix,(ann is Annotation)).toString()\n",
    "sourceSha256": "3dcddfa91a574d39bd72a6482d05ed0162423e05c1b2229138763c4d04b2f26f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-source-code-defaults",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7dbfcd52b44fc7c7d66634683dde04e1603217bf1e099ec4e54106c3a19a6bb2",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nTried to read property `language` but its value is undefined.\n\n210 | language: \"Go\"\n      ^^^^^^^^\nat pkl.base#SourceCode.language (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L210)\n\n2 | result=List(ann.language,ann.prefix,ann.suffix,(ann is Annotation)).toString()\n                ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-source-code-union-and-string",
    "source": "result=List(\"Go\",\"HTML\",\"Java\",\"JavaScript\",\"Markdown\",\"Pkl\",\"PklExpr\",\"Python\",\"Ruby\",\"SQL\",\"Swift\",\"Rust\").map((s)->new SourceCode { language=s }.language).toString()\n",
    "sourceSha256": "2f37aec577251c887aa097cf8b59a1248e93e7514c6ada1f824f5ecc7f421fe2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-source-code-union-and-string",
    "captureSha256": {
      "stdout": "84dbd6175e6b8baebdd50ea08e65e62ee673702e2ad82bd4fd2c2da9a27db677",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(\"Go\", \"HTML\", \"Java\", \"JavaScript\", \"Markdown\", \"Pkl\", \"PklExpr\", \"Python\", \"Ruby\", \"SQL\", \"Swift\", \"Rust\")"
  },
  {
    "id": "annotation-stage35-builtin-source-code-language-wrong-type",
    "source": "ann=new SourceCode { language=1 }\nresult=ann.language.toString()\n",
    "sourceSha256": "d1e31dad570691f3a4ef92cb9c35d52e982687daacf8daebbcdf576566c9c887",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-source-code-language-wrong-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4956ca82441ef8146b0d439f412d5862485419b502d7ceca37b902276e7e13bf",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `\"Go\"\n    | \"HTML\"\n    | \"Java\"\n    | \"JavaScript\"\n    | \"Markdown\"\n    | \"Pkl\"\n    | \"PklExpr\"\n    | \"Python\"\n    | \"Ruby\"\n    | \"SQL\"\n    | \"Swift\"\n    | String`, but got type `Int`.\nValue: 1\n\n210 | language: \"Go\"\n                ^^^^\nat pkl.base#SourceCode.language (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L210)\n\n1 | ann=new SourceCode { language=1 }\n                                  ^\nat m2-platform#ann.language (file:///m2-platform.pkl)\n\n2 | result=ann.language.toString()\n           ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-source-code-prefix-wrong-type",
    "source": "ann=new SourceCode { prefix=1 }\nresult=ann.prefix.toString()\n",
    "sourceSha256": "ef95b6124dc1dbefcf8ac9c30c37b306f0ef03b9ca22ee407ce392b5a9b93de8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-source-code-prefix-wrong-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "cb377b2f52d27e0e345b9263e1bd262f0488135fe760753d258616d85e24df55",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n226 | prefix: String?\n              ^^^^^^\nat pkl.base#SourceCode.prefix (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L226)\n\n1 | ann=new SourceCode { prefix=1 }\n                                ^\nat m2-platform#ann.prefix (file:///m2-platform.pkl)\n\n2 | result=ann.prefix.toString()\n           ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "annotation-stage35-builtin-custom-annotation-hierarchy",
    "source": "open class Custom extends Annotation\nclass Child extends Custom\n@Child\nclass C {}\nann=new Child {}\nresult=List((ann is Annotation),(ann is Custom),(ann is Child)).toString()\n",
    "sourceSha256": "a6bd06cb808eca4e62bf42f83f7d7281a00a05d3b736c37f9108250a4e350ba2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-custom-annotation-hierarchy",
    "captureSha256": {
      "stdout": "898a867e248868a353c9bcddb9e510a7bebcbfecfad475a1ad15c546ff62979f",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(true, true, true)"
  },
  {
    "id": "annotation-stage35-builtin-source-code-null-defaults",
    "source": "ann=new SourceCode { language=\"Go\" }\nresult=List(ann.prefix,ann.suffix,(ann is Annotation)).toString()\n",
    "sourceSha256": "2b732c427cba2e4e58fe2a96f2765a6d42ce58bff3119c2b9e0c7934e5d85775",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/recovered/cases/annotation-stage35-builtin-source-code-null-defaults",
    "captureSha256": {
      "stdout": "ecf7d9f005dc08a9f57586b8b824e9c96c8435c3ef74d750358eb6372f7f0326",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(null, null, true)"
  },
  {
    "id": "annotation-stage35-builtin-source-code-union-direct",
    "source": "result=List(new SourceCode { language=\"Go\" }.language,new SourceCode { language=\"HTML\" }.language,new SourceCode { language=\"Java\" }.language,new SourceCode { language=\"JavaScript\" }.language,new SourceCode { language=\"Markdown\" }.language,new SourceCode { language=\"Pkl\" }.language,new SourceCode { language=\"PklExpr\" }.language,new SourceCode { language=\"Python\" }.language,new SourceCode { language=\"Ruby\" }.language,new SourceCode { language=\"SQL\" }.language,new SourceCode { language=\"Swift\" }.language,new SourceCode { language=\"Rust\" }.language).toString()\n",
    "sourceSha256": "50fe1406573c59350632909f1d7ac763f6c1c3d7c64b6d9818ca109f8a9cdd43",
    "expression": "result",
    "oracleCasePath": "target/m2/stage35-builtin-selected-oracle-01/direct/cases/annotation-stage35-builtin-source-code-union-direct",
    "captureSha256": {
      "stdout": "84dbd6175e6b8baebdd50ea08e65e62ee673702e2ad82bd4fd2c2da9a27db677",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "List(\"Go\", \"HTML\", \"Java\", \"JavaScript\", \"Markdown\", \"Pkl\", \"PklExpr\", \"Python\", \"Ruby\", \"SQL\", \"Swift\", \"Rust\")"
  }
];

export const annotationCaseCount = cases.length;

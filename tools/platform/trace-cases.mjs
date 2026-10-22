// Exact pinned Java UTF-8 scalar values and errors, without normalization.
// Logger events and ordered source-session calls are separate evidence.
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
        "reportPath": "target/m2/stage37-selected-oracle-01/report.json",
        "reportSha256": "6eea359d2b4fb451e42faeec0fe7554a60c39e4f34b26bae472501f1509f896f",
        "payloadPath": "target/m2/stage37-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "c44840328def510805972c82f3bc8e49f172209616074b5d8476e97bfe05ba08",
        "cases": 18
      }
    ]
  }
};

export const cases = [
  {
    "id": "trace-stage37-scalar-cache",
    "source": "result=trace( (1 + 2) )\n",
    "sourceSha256": "126f9fe3f63fa0aa43791cdf60882aeaaf5aff08af473d93e1b2ae34b1d184b4",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-scalar-cache",
    "captureSha256": {
      "stdout": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "3"
  },
  {
    "id": "trace-stage37-operand-let",
    "source": "result=trace( ((let (x=1) (x + 2))) )\n",
    "sourceSha256": "81be024661bf664de4c6dc64760bba83c80762841b6c40ebf4ebb6111a7a796d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-operand-let",
    "captureSha256": {
      "stdout": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "3"
  },
  {
    "id": "trace-stage37-nested-trace-call",
    "source": "result=trace( (trace( (1 + 2) )) )\n",
    "sourceSha256": "f5faf5ef957a66ac29e6bf28f4e1b5c81823e585643466c81adc7c2a3073f149",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-nested-trace-call",
    "captureSha256": {
      "stdout": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "3"
  },
  {
    "id": "trace-stage37-source-utf16",
    "source": "prefix=\"\ud83d\ude00\"; result=trace(\"\ud83d\ude00\")\n",
    "sourceSha256": "d0df6fc1cf541a956d4cd9cf6c5c62eafcb15edfdeb50c81e7b61e9d26197bfa",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-source-utf16",
    "captureSha256": {
      "stdout": "f0443a342c5ef54783a111b51ba56c938e474c32324d90c3a60c9c8e3a37e2d9",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "\ud83d\ude00"
  },
  {
    "id": "trace-stage37-unpaired-utf16",
    "source": "result=trace(\"\\u{D800}\")\n",
    "sourceSha256": "f546435a9f5848b16a29847b6038c60bc6696d030a4d3a36a53eede524cd64b8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-unpaired-utf16",
    "captureSha256": {
      "stdout": "8a8de823d5ed3e12746a62ef169bcf372be0ca44f0a1236abc35df05d96928e1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "?"
  },
  {
    "id": "trace-stage37-inferred-property",
    "source": "class C { x=7 }\nfoo: C=trace(new {})\nresult=foo.x\n",
    "sourceSha256": "5c86d73956d692f1da59ec951dc2c79a405cf0e978dd970b7d217acc480d2a0c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-inferred-property",
    "captureSha256": {
      "stdout": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "7"
  },
  {
    "id": "trace-stage37-inferred-method-let",
    "source": "class C { x=7 }\nfunction make(): C=trace(let (n=1) new { x=n })\nresult=make().x\n",
    "sourceSha256": "83d77ad0136ea0d16f0c38138d98f4efb71224d2f9c0c6637a35ac98a8c20be4",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-inferred-method-let",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "trace-stage37-inferred-group-error",
    "source": "class C { x=7 }\nfoo: C=trace((new {}))\nresult=foo.x\n",
    "sourceSha256": "267771ec305b0b735363324e870a92aca87dc2c8cc3a7c23c1b3a24340aa07a6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-inferred-group-error",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e9d509af2efde431a076371e44b30433bd1a567ebea8775f235a6ab439ab56a6",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | foo: C=trace((new {}))\n                  ^^^\nat m2-platform#foo (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "trace-stage37-operand-error",
    "source": "result=trace(throw(\"operand\"))\n",
    "sourceSha256": "639aa880e5db514ce604aa2572aea69b88ac2208e5dae52e0b672443b779b587",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-operand-error",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "855a7b87a59d2e8bbdc2a0bac7de03df51870f4cc89cdbcd06e626ac019a1757",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\noperand\n\n1 | result=trace(throw(\"operand\"))\n                 ^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-noop-force-error",
    "source": "obj=new Dynamic { x=throw(\"forced\") }\nresult=trace(obj).x\n",
    "sourceSha256": "e75fa940fa200c773dd818657372930a43c3c798b2f35cd9609a03ff054f6b29",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-noop-force-error",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "766eab509b0aad78232fef52965047bd0bc56baa6f4dbfec73e67af603f575f1",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nforced\n\n1 | obj=new Dynamic { x=throw(\"forced\") }\n                        ^^^^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-noop-force-read",
    "source": "obj=new Dynamic { x=read(\"life:text\") }\nresult=trace(obj).x\n",
    "sourceSha256": "1c85996c2436614f167d80bc6de9f8ec02f34b006116c574d9f7bcbdc65dc612",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-noop-force-read",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "72f679bab73d460a32e33dc10949543c67cd2718df9af1db3d6f361489214826",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nRefusing to read resource `life:text` because it does not match any entry in the resource allowlist (`--allowed-resources`).\n\n1 | obj=new Dynamic { x=read(\"life:text\") }\n                        ^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-undefined-direct",
    "source": "class C { x:Int }\nobj=new C {}\nresult=trace(obj.x)\n",
    "sourceSha256": "89c3971b78d4ec8ddb5df0e1bb2b57b9fe3373b9a2fdb9b1aa4141d2bd851ea6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-undefined-direct",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a21aa018c346288aeb3b28a2e1eb56b6430735496523fb261bed14d8a6e2a977",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `x` but its value is undefined.\n\n1 | class C { x:Int }\n              ^\nat m2-platform#C.x (file:///m2-platform.pkl)\n\n3 | result=trace(obj.x)\n                 ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-undefined-object",
    "source": "class C { x:Int }\nobj=new C {}\nfirst=trace(obj)==obj\nsecond=trace(obj)==obj\nmissing=obj.x\n",
    "sourceSha256": "987a8117eef55926e1849b12649dfecf240d158411e0ae19b1ceb50d3b0f928a",
    "expression": "first",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-undefined-object",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "trace-stage37-override-ignored",
    "source": "class C { x=1; function toString():String=throw(\"override\") }\nobj=new C {}\nresult=trace(obj).x\n",
    "sourceSha256": "cb8cf5eafd66677c8c8c09b1abae1544d082e27d1eeed9689f02a89eacf1afa0",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-override-ignored",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "trace-stage37-strict-list-force",
    "source": "obj=new Dynamic { x=read(\"life:text\") }\nresult=trace(List(obj)).length\n",
    "sourceSha256": "3ff701661b34d8e5715c270226a7977fe5055baa4f6fc4f8cf9cabd03b5d48c9",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-strict-list-force",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "72f679bab73d460a32e33dc10949543c67cd2718df9af1db3d6f361489214826",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nRefusing to read resource `life:text` because it does not match any entry in the resource allowlist (`--allowed-resources`).\n\n1 | obj=new Dynamic { x=read(\"life:text\") }\n                        ^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-pretty-multiline",
    "source": "result=trace(new Dynamic {\n  x=1\n  nested { y=2 }\n}).x\n",
    "sourceSha256": "5a3fc0a4d06ffafe99d7ac9e12a4110480a25687aa488470fb1d0a9b8fc36366",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-pretty-multiline",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "trace-stage37-pretty-crlf",
    "source": "result=trace(new Dynamic {\r\n  x=1\r\n}).x\r\n",
    "sourceSha256": "2c859f7c8957d7b6ac093c55ba2e7211a1cae4eae5a84be8465d58c847ca1b6f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-pretty-crlf",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1"
  },
  {
    "id": "trace-stage37-compact-limit",
    "source": "result=trace(\"a\".repeat(1000010)).length\n",
    "sourceSha256": "2eda9d2dc2484254cb6ce9e27a6e087b3ce36e69edd930ee1c41c7295b631556",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-selected-oracle-01/cases/trace-stage37-compact-limit",
    "captureSha256": {
      "stdout": "a9c77f1c964d34365e0f5bcb4bdbf8ee95cb562d7e7d3c658568a5f1b07739cc",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1000010"
  }
];

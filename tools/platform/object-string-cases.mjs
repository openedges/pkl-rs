// Exact pinned Java UTF-8 values and errors, without normalization.
export const context = {
  "moduleName": "m2-platform",
  "oracle": {
    "artifactSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "lockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "version": "0.32.0",
    "acquisitions": [
      {
        "reportPath": "target/m2/stage36-selected-oracle-01/report.json",
        "reportSha256": "26c67ef2c45a4bedb41495f415e9514480370c7955b2b38a7bc1db835136f1a3",
        "payloadPath": "target/m2/stage36-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "e1a82dba45b87dc37244826345af44932a5210996eed6bbe652788ca9c3f95db",
        "cases": 68
      },
      {
        "reportPath": "target/m2/stage36-owner-hidden-selected-oracle-01/report.json",
        "reportSha256": "1c48fbb05cf2c4d25d84d9280aba624099c77b6b45ecb47327dd3e35ec12b3d7",
        "payloadPath": "target/m2/stage36-owner-hidden-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "d1b7cf7f703b836efc009c1808bbba5a23b87924128a13dff6cefcf73d3fca07",
        "cases": 2
      },
      {
        "reportPath": "target/m2/stage36-review-selected-oracle-01/report.json",
        "reportSha256": "586463c73f1f6287d002ef05f8dffe0030d9023dae9b2512351132f45f347b3a",
        "payloadPath": "target/m2/stage36-review-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "a4a0ae9aa5f6ec2a9404043597758a8345b319d8e24b626a3feb60da4b1e9fb4",
        "cases": 9
      },
      {
        "reportPath": "target/m2/stage36-review-collision-selected-oracle-01/report.json",
        "reportSha256": "1b93f4d9a3f1d86cfb45af108088b268e52c63954018394a79ef91b86e8c2f9d",
        "payloadPath": "target/m2/stage36-review-collision-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "c954d8771d2a7a18fe3a78b2fce2b70b2f0ec27972185ea41d5a55cd21edcbdd",
        "cases": 1
      },
      {
        "reportPath": "target/m2/stage36-review-modifier-selected-oracle-01/report.json",
        "reportSha256": "9ca3aaca52cd1bbc00231fe5376b139c535c15d98b2e4cb58a3efdc11529e5e9",
        "payloadPath": "target/m2/stage36-review-modifier-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "dadf7bb5b9efe8a55fc58fbfc98d07d53fefba49b201892c86fbf116a0605c2c",
        "cases": 3
      }
    ]
  },
  "powerAssertions": false,
  "uri": "file:///m2-platform.pkl"
};

export const cases = [
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "49c7221f73f2b39609ff8f9bb23ee3c964c3378b93a7e4c20929545b7d9e03bf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nchild\n\n1 | obj = (new Dynamic { a=throw(\"parent\") }) { b=throw(\"child\") }\n                                                  ^^^^^^^^^^^^^^\nat m2-platform#obj.b (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-dynamic-amend-force-child-first",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-amend-force-child-first",
    "source": "obj = (new Dynamic { a=throw(\"parent\") }) { b=throw(\"child\") }\nresult=obj.toString()\n",
    "sourceSha256": "19286abb787bfc9f9809f562dcb694859bdbb1572e416144a229ba347bf85be1"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "75a25d9607db0bb6ba29388aadca18a979acc06d0025ef26bb5695f67b33a52c"
    },
    "expectedValue": "new Dynamic { a = 10; b = 20; c = 3 }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-amend-render-order",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-amend-render-order",
    "source": "obj = (new Dynamic { a=1; b=2 }) { b=20; c=3; a=10 }\nresult=obj.toString()\n",
    "sourceSha256": "d9d9b44bcb01b3349310f5e82804d049fd9feee4b7076f21f1ea10c8153c11c0"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "71d8c2387bc3a6c177bf6cb982cfc051962e77ae253a0ec740077ddb99a631ce"
    },
    "expectedValue": "new Dynamic { 1; \"two\"; new Dynamic { value = 7 } }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-elements",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-elements",
    "source": "obj = new Dynamic { 1; \"two\"; new Dynamic { value=7 } }\nresult=obj.toString()\n",
    "sourceSha256": "9ead63ccfbabbe2b441c43c7377e11d426064a92b1cad9d95d380fa9fd0cca35"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "6948d378b5db7417650c4ecf402848506a6ea9f9fd88fd0933b521f649272507"
    },
    "expectedValue": "new Dynamic {}",
    "expression": "result",
    "id": "object-string-stage36-dynamic-empty",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-empty",
    "source": "obj = new Dynamic {}\nresult=obj.toString()\n",
    "sourceSha256": "6d298cbb893a4612b5930d7d2cdeda4146a17ef3889fcd30bc70eff555242849"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "9d604c48875fa55417cdd54517af48f739c8ecd625fb2b6caff0dcbcc19d82cf"
    },
    "expectedValue": "new Dynamic { [\"one\"] = 1; [2] { value = 7 } }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-entry",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-entry",
    "source": "obj = new Dynamic { [\"one\"]=1; [2]=new Dynamic { value=7 } }\nresult=obj.toString()\n",
    "sourceSha256": "93dc2dfc912142c35e4046e51a8a92c9576af51d6057daeb685a386c52808cbb"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "e5a77199d79fcd91c5221730e391676743fd54a4cf53cb0f8bc16916875c0242"
    },
    "expectedValue": "new Dynamic { z = 1; a = 4; [\"key\"] = 2; 3 }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-mixed-order",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-mixed-order",
    "source": "obj = new Dynamic { z=1; [\"key\"]=2; 3; a=4 }\nresult=obj.toString()\n",
    "sourceSha256": "5b938c0a4a6fb6dbd8d9e3d96306d306d369f2dc96cb20e09b31c77312ec284f"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "4079b909e5a99bcee114e86e00ebc91997f6e2eab4bf6f731c926419ebe29f9c"
    },
    "expectedValue": "new Dynamic { child { value = 7 } }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-nested-property",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-nested-property",
    "source": "obj = new Dynamic { child=new Dynamic { value=7 } }\nresult=obj.toString()\n",
    "sourceSha256": "42623ea893d7521017cf938160522b3d1914aa3c2427fdb7005dfbfa1b080289"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "17c61eb9ceea0de1ea78723076538e157b4701091642c1a8edae975bc5676341"
    },
    "expectedValue": "new Dynamic { x = 1; y = \"two\"; z = true; n = null }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-scalars",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-scalars",
    "source": "obj = new Dynamic { x=1; y=\"two\"; z=true; n=null }\nresult=obj.toString()\n",
    "sourceSha256": "0b88ac07fe8aab2eb6ed803ba3e33f9e6ef9d497686bca5f73266357374ee438"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "9e2f64f637e2e437eed6a2127fabf80d47d70b1b732afcb8a88db065fb6cb45c",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nkey\n\n1 | obj = new Dynamic { [new Dynamic { x=throw(\"key\") }]=1 }\n                                         ^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-entry-key-object-nonforcing",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-entry-key-object-nonforcing",
    "source": "obj = new Dynamic { [new Dynamic { x=throw(\"key\") }]=1 }\nresult=obj.toString()\n",
    "sourceSha256": "c1de34fa551d4bd78bb6e4f79ad28f71f5e1d2dffc410380229b92840f2438fd"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "8db1f0b29f72f9662927f302e1754abda4e6d7e6ff778d9014d7ff931d20f343"
    },
    "expectedValue": "new Dynamic { f {}; klass = Dynamic; alias = Int8 }",
    "expression": "result",
    "id": "object-string-stage36-function-and-class-values",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-function-and-class-values",
    "source": "obj = new Dynamic { f=(x)->x; klass=Dynamic; alias=Int8 }\nresult=obj.toString()\n",
    "sourceSha256": "e1cf426ee7d6980adcafaf719f7b724032d4c93ca21d3019cf472dd479f29fb2"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "3d9652ef3d87037524edc4fd9c069e150970384f3cb1b238b77256a71ebf99dd"
    },
    "expectedValue": "new Dynamic { `class` = 1; `has space` = 2; `two-words` = 3; caf\u00e9 = 4 }",
    "expression": "result",
    "id": "object-string-stage36-identifier-escaping",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-identifier-escaping",
    "source": "obj = new Dynamic { `class`=1; `has space`=2; `two-words`=3; caf\u00e9=4 }\nresult=obj.toString()\n",
    "sourceSha256": "57b0165232611670f2de940330bf34fd425875083ce76ddd939dd7d4be8696c1"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "5b5601f969a4fbbb1514d41e0e5802e49d711abe87cfb2dd7805de6a670c3c7c",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nchild\n\n2 | class C extends B { child=throw(\"child\") }\n                              ^^^^^^^^^^^^^^\nat m2-platform#C.child (file:///m2-platform.pkl)\n\n3 | result=new C {}.toString()\n           ^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-inherited-force-child-first",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-inherited-force-child-first",
    "source": "open class B { inherited=throw(\"parent\") }\nclass C extends B { child=throw(\"child\") }\nresult=new C {}.toString()\n",
    "sourceSha256": "82e45f2fa00a4b9cefd27ce7e2719d441a8558658866e3d6bf443518e273e373"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a3240ca85f71f61ec24a058da6c104ab4c2ee31771b5123341bd5dc196e95ac0"
    },
    "expectedValue": "new C {}",
    "expression": "result",
    "id": "object-string-stage36-inherited-hidden-override",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-inherited-hidden-override",
    "source": "open class B { hidden x=throw(\"hidden\") }\nclass C extends B { x=throw(\"override\") }\nresult=new C {}.toString()\n",
    "sourceSha256": "3288880592c96dd33f639c9f7193316240ac18e5bc1590d5449cb3a3f359b94a"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a0fb75a230840f418a3848c2d5e81aaf2e5102a162d9b03ead71a055a8ceed41"
    },
    "expectedValue": "new C { x = 1 }",
    "expression": "result",
    "id": "object-string-stage36-inherited-override-no-parent-force",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-inherited-override-no-parent-force",
    "source": "open class B { x=throw(\"parent\") }\nclass C extends B { x=1 }\nresult=new C {}.toString()\n",
    "sourceSha256": "f35d9bac4464e36438771705588e3ad139eebcdfef21205a62f0cebd81f257fd"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "09f098d5d7346210d127d58f9acfa3dee649296ef50e3206968252fbb389e743"
    },
    "expectedValue": "new C { first = 10; second = 20; third = 3 }",
    "expression": "result",
    "id": "object-string-stage36-inherited-render-order",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-inherited-render-order",
    "source": "open class B { first=1; second=2 }\nclass C extends B { second=20; third=3; first=10 }\nresult=new C {}.toString()\n",
    "sourceSha256": "f609ad8067f44e8c7f039f1c844e94a667c3ca00ccdbe1bf9db17ed84ae32180"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "341fa45e6d377518f1fab4b3447ef1f16c87530c761fbfb65229e0f7eb18df6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nfirst\n\n1 | obj = new Dynamic { a=throw(\"first\"); b=throw(\"second\") }\n                          ^^^^^^^^^^^^^^\nat m2-platform#obj.a (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-lazy-first-error",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-lazy-first-error",
    "source": "obj = new Dynamic { a=throw(\"first\"); b=throw(\"second\") }\nresult=obj.toString()\n",
    "sourceSha256": "2be6f6f0a5c14fb8512e217215f3ad3341965dd22754fea7221e83e2c63d4285"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "2f88a0ac8b7e7ebab87e4f23b35456130f318df6b5ee5227c505a4cf87753548"
    },
    "expectedValue": "new Listing {}",
    "expression": "result",
    "id": "object-string-stage36-listing-empty",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-listing-empty",
    "source": "obj = new Listing {}\nresult=obj.toString()\n",
    "sourceSha256": "067b27c3e1e99416fa6126d14b05a44fec3949120c59c2638c8164858c151bdd"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "0433916df90fe79bf22f175efa1b19ca642f37d1f2bef13152811040d6193e65"
    },
    "expectedValue": "new Listing { new Dynamic { x = 1 }; new Listing { 2 }; new Mapping { [\"a\"] = 3 } }",
    "expression": "result",
    "id": "object-string-stage36-listing-nested",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-listing-nested",
    "source": "obj = new Listing { new Dynamic { x=1 }; new Listing { 2 }; new Mapping { [\"a\"]=3 } }\nresult=obj.toString()\n",
    "sourceSha256": "70c904b64a5d40280c897a2deea3d04cc45d1ca24ed63b745200028b10ec0dbf"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "305b3d5f257a571994604cced43632359a555fed87834c5d102ea71a30b228b3"
    },
    "expectedValue": "new Listing { 1; \"two\"; true; null }",
    "expression": "result",
    "id": "object-string-stage36-listing-values",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-listing-values",
    "source": "obj = new Listing { 1; \"two\"; true; null }\nresult=obj.toString()\n",
    "sourceSha256": "8421d0f27024a2993254e0343bd025e24a76787b67916ac212c43d38b289e44d"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "b0a43c05fe93654911f1c29de5907a4414f950dfd22f9ade4834d3689381db93"
    },
    "expectedValue": "new Mapping {}",
    "expression": "result",
    "id": "object-string-stage36-mapping-empty",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-mapping-empty",
    "source": "obj = new Mapping {}\nresult=obj.toString()\n",
    "sourceSha256": "639f0042d467e787da6164e1aab167d71ed0334e87d7272b7f5d57318a067775"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "2af0906fd97d216be6388ec5e00516af3dc5ac98fa76d3bc00127291f3575908"
    },
    "expectedValue": "new Mapping { [\"d\"] { x = 1 }; [\"l\"] { 2 } }",
    "expression": "result",
    "id": "object-string-stage36-mapping-nested",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-mapping-nested",
    "source": "obj = new Mapping { [\"d\"]=new Dynamic { x=1 }; [\"l\"]=new Listing { 2 } }\nresult=obj.toString()\n",
    "sourceSha256": "92efe1bcfc6404a6916d717c72bb49eea4a0c553280b2b24a440e0df0c772de1"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "7e4316076c6dd4f425ed7fb045ce35d37f5f9f3bbe728870c5ee6247db7ca6de"
    },
    "expectedValue": "new Mapping { [\"a\"] = 1; [2] = \"two\" }",
    "expression": "result",
    "id": "object-string-stage36-mapping-values",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-mapping-values",
    "source": "obj = new Mapping { [\"a\"]=1; [2]=\"two\" }\nresult=obj.toString()\n",
    "sourceSha256": "e753489582717478fc360897f1c86c5153da7e824473aedb061ffa210ed3ca69"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "43eef892db90641b1c1631e277b1fa43aa18e7b0b7cf3e430d2e872996b96847"
    },
    "expectedValue": "new ModuleClass {}",
    "expression": "result",
    "id": "object-string-stage36-module-empty",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-module-empty",
    "source": "hidden result=module.toString()\n",
    "sourceSha256": "8c6d3cfb244ef7ef01a0b0ca58c614e3d9219440a42b3123d77e8d1a1d860c7c"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "6f59c3b194c3140289d08ab435d42546a1191caf5a420cb36a092323fa5b6de7",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected an annotation class.\n\n1 | @Int\n     ^^^\nat m2-platform#C (file:///m2-platform.pkl)\n\n4 | hidden result=module.toString()\n                  ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-module-force-class-error",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-module-force-class-error",
    "source": "@Int\nclass C {}\nx=1\nhidden result=module.toString()\n",
    "sourceSha256": "bf773a0e917c12fe67ed5f4973541c2dc0ab04806b391024539240ebe631ba63"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "b0fc04d5db2a63b230330308cef85cdef88ac47ae2de7f63fe02a5c7b9ff1129"
    },
    "expectedValue": "new ModuleClass { result = \"new ModuleClass { result = ? }\" }",
    "expression": "result",
    "id": "object-string-stage36-module-self-result",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-module-self-result",
    "source": "result=module.toString()\n",
    "sourceSha256": "7ac382401caa114a4bc21480ce989e6075381ad6d6929de9ba9c99e8b3099fbb"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "ea93bf2a71b29b9de8d4eff5212f492314c3dc53482610745cc9d3de8a113adf"
    },
    "expectedValue": "new ModuleClass { x = 1 }",
    "expression": "result",
    "id": "object-string-stage36-module-skip-definitions",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-module-skip-definitions",
    "source": "class C {}\ntypealias A=Int\nx=1\nfunction f()=throw(\"method\")\nlocal private=throw(\"local\")\nhidden secret=throw(\"hidden\")\nhidden result=module.toString()\n",
    "sourceSha256": "be1e9c5fc7293185294e8c36c1c27d82310856a0c8a4b61f0a54051f0159589d"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "b99981b921804c5b4274a442e69e7abcfdafdcfc5a5f6fbe71090c6b77f824e0"
    },
    "expectedValue": "new ModuleClass { x = 1; y = \"two\" }",
    "expression": "result",
    "id": "object-string-stage36-module-values",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-module-values",
    "source": "x=1\ny=\"two\"\nhidden result=module.toString()\n",
    "sourceSha256": "ab5c4438f4ca5ae9dbad0e737b0bb0c14dfdcb3e55f471f19898697751f62e6e"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "c31433b67a1cb3076c596251a6cfc602437d8fc093584a6a336d55598cccc927",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nnested\n\n1 | obj = new Dynamic { x=new Dynamic { value=throw(\"nested\") }; y=throw(\"later\") }\n                                              ^^^^^^^^^^^^^^^\nat m2-platform#obj.x.value (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-nested-error-before-next",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-nested-error-before-next",
    "source": "obj = new Dynamic { x=new Dynamic { value=throw(\"nested\") }; y=throw(\"later\") }\nresult=obj.toString()\n",
    "sourceSha256": "1fdbb9e9e6440d816ee599acd4b8ba8f2024a00d1be2473dbf6ec075c2c9f55b"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "67d53c4103c5dd6c9b4fdd36b494979752bbc115764bb9eb34bf6883c3144c53",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nnested list\n\n1 | obj = new Dynamic { x=List(new Dynamic { value=throw(\"nested list\") }) }\n                                                   ^^^^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x.value (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-nested-strict-list-forced",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-nested-strict-list-forced",
    "source": "obj = new Dynamic { x=List(new Dynamic { value=throw(\"nested list\") }) }\nresult=obj.toString()\n",
    "sourceSha256": "5f61d8d14f9ab593c1dbe8637c0c95d8f065de7478fc1b1390700d09199d7419"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "cda9ec4ec26cc33d6b669a9f94e2aabda8a3921af8b56c1ceda4f9ab7aaa3a0a",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nnested key\n\n1 | obj = new Dynamic { x=Map(new Dynamic { value=throw(\"nested key\") },1) }\n                                                  ^^^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x.value (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-nested-strict-map-keys-forced",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-nested-strict-map-keys-forced",
    "source": "obj = new Dynamic { x=Map(new Dynamic { value=throw(\"nested key\") },1) }\nresult=obj.toString()\n",
    "sourceSha256": "4fc0b7a79dd7ff5b96b82bf0a598bc836e3c890f795ec3627ce77d2527158db7"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "e21b106488a53dc6326a21ed5588a81d26abb511a2baf80afccab0454a908c9e",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nnested value\n\n1 | obj = new Dynamic { x=Map(\"a\",new Dynamic { value=throw(\"nested value\") }) }\n                                                      ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x.value (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-nested-strict-map-values-forced",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-nested-strict-map-values-forced",
    "source": "obj = new Dynamic { x=Map(\"a\",new Dynamic { value=throw(\"nested value\") }) }\nresult=obj.toString()\n",
    "sourceSha256": "73e2cbfd318525510fdfbf09c9176e42c2845566155b7223836fe2d3b71c08f0"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "736cfcdca5485efed0d4b078c3f1436f7b7960cb5cd2157cccd0ded2f7b8a566"
    },
    "expectedValue": "List(new Dynamic {\n  x = ?\n})",
    "expression": "result",
    "id": "object-string-stage36-strict-list-control-unforced",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-strict-list-control-unforced",
    "source": "result=List(new Dynamic { x=throw(\"unforced\") }).toString()\n",
    "sourceSha256": "ae2d20ee6ae00851d669d3d345809f8030119b96c24c7c906e840a7c63ddac68"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "9526ebc9d6060828f512e0ed21d5b6748d6e07c7077302c9d40e8e3cb5538747"
    },
    "expectedValue": "Map(\"a\", new Dynamic { x = ? })",
    "expression": "result",
    "id": "object-string-stage36-strict-map-control-unforced",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-strict-map-control-unforced",
    "source": "result=Map(\"a\",new Dynamic { x=throw(\"unforced\") }).toString()\n",
    "sourceSha256": "10bea4716d76640bc09f43fc39ddffa97e911b3478825ca2339a3be25e83f993"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "7bc673165be9e409bffcb1f95236c1f104bfceb5ca6d1190f86bc4f4c3f20eb9"
    },
    "expectedValue": "new Dynamic { text = \"quote\\\" slash\\\\ tab\\t line\\n return\\r null\u0000\" }",
    "expression": "result",
    "id": "object-string-stage36-string-escaping",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-string-escaping",
    "source": "obj = new Dynamic { text=\"quote\\\" slash\\\\ tab\\t line\\n return\\r null\\u{0}\" }\nresult=obj.toString()\n",
    "sourceSha256": "e46066bfe27aeb1c9e094c6c3e9973a999e7e68dadafcb4a48fb5ea0dfaf9898"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "4552f5dbb0dd0e0e4d505d901c5a750ae7a876ab37d9e60e762c0e8e134011e7"
    },
    "expectedValue": "new Dynamic { a = \"?\"; b = \"?\"; c = \"\ud83d\ude00\"; d = \"\ud83d\ude00\" }",
    "expression": "result",
    "id": "object-string-stage36-string-utf16",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-string-utf16",
    "source": "obj = new Dynamic { a=\"\\u{d800}\"; b=\"?\"; c=\"\ud83d\ude00\"; d=\"\\u{d83d}\\u{de00}\" }\nresult=obj.toString()\n",
    "sourceSha256": "53af31c28aa61afd3ce4f5573551f2fbdd29bef9b8f15243f60fc49745eb89df"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "efd40fce9676c1637cdff70c1cd35138c08f255db1c18a19d8648e8a2d1f7eaa"
    },
    "expectedValue": "new Dynamic { x = 1 }",
    "expression": "result",
    "id": "object-string-stage36-supplement-dynamic-local-method-corrected",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-dynamic-local-method-corrected",
    "source": "obj=new Dynamic { x=1; local private=throw(\"local\"); local function f()=throw(\"method\") }\nresult=obj.toString()\n",
    "sourceSha256": "ba506cd2018556ff48bb5544600f32e6cea9cf22cb5ad0543ee3a1b0f7f3b58c"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "70a49155951bf1b97d8414a32018bd2e9c38f41bec8bb1334e802cc694d5ea12",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nkey\n\n1 | obj=new Mapping { [new Dynamic { x=throw(\"key\") }]=1 }\n                                       ^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-supplement-mapping-key-object-snapshot",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-mapping-key-object-snapshot",
    "source": "obj=new Mapping { [new Dynamic { x=throw(\"key\") }]=1 }\nresult=obj.toString()\n",
    "sourceSha256": "2c11fa7e1928617a39fd70c2efe7994db2e9b6a999b93217bb246211acabf699"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "e1d5bb20ba8fac28f15c05186444e1d81a82de92dc2d8eeac4ebc522ede96a91",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nprop\n\n1 | obj=new Dynamic { throw(\"element\"); prop=throw(\"prop\") }\n                                             ^^^^^^^^^^^^^\nat m2-platform#obj.prop (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-supplement-mixed-first-error-property-before-element",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-mixed-first-error-property-before-element",
    "source": "obj=new Dynamic { throw(\"element\"); prop=throw(\"prop\") }\nresult=obj.toString()\n",
    "sourceSha256": "a983f4f240ddf7eb291ddf32b63f1ea25082775ee728a8b5e3d1019ba3dc0f39"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "4abd133920520ca60d911b9541ed5fa46fe31face8d2f02ad9d457dcfa452b59",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nprop\n\n1 | obj=new Dynamic { [\"entry\"]=throw(\"entry\"); prop=throw(\"prop\") }\n                                                     ^^^^^^^^^^^^^\nat m2-platform#obj.prop (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-supplement-mixed-first-error-property-before-entry",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-mixed-first-error-property-before-entry",
    "source": "obj=new Dynamic { [\"entry\"]=throw(\"entry\"); prop=throw(\"prop\") }\nresult=obj.toString()\n",
    "sourceSha256": "2d6853c639379791399d7df21bdbc2c7e9b70f9a04814a6ccaeab1bebfc8fa91"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "bf875a73b7839a3b9a1798040a8cc65d2ab51f8e84438f9d23700e8f943e3f4e"
    },
    "expectedValue": "new Dynamic { c { x = 1 }; list = List(new C { x = 1 }); map = Map(\"c\", new C { x = 1 }); listing { new C { x = 1 } }; mapping { [\"c\"] { x = 1 } } }",
    "expression": "result",
    "id": "object-string-stage36-supplement-nested-render-context-mixed",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-nested-render-context-mixed",
    "source": "class C { x=1 }\nobj=new Dynamic { c=new C {}; list=List(new C {}); map=Map(\"c\",new C {}); listing=new Listing { new C {} }; mapping=new Mapping { [\"c\"]=new C {} } }\nresult=obj.toString()\n",
    "sourceSha256": "2bc6de4d31d90d0e4c4d06aaf71bca11a64b2565ddff0ebff3bff9b82ac7ae20"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "39dd6e62eb048c4ed25ee9436e8c5b272c578fd4585b58a82c9fdb0e204e7725"
    },
    "expectedValue": "new Dynamic { first { value = 1 }; second { value = 1 } }",
    "expression": "result",
    "id": "object-string-stage36-supplement-shared-child-finite",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-shared-child-finite",
    "source": "child=new Dynamic { value=1 }\nobj=new Dynamic { first=child; second=child }\nresult=obj.toString()\n",
    "sourceSha256": "21e9fd24e9cfc0449f6b1b7b4ce97d443e43668eba93dbd001e7ad168f2de515"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "21cd58f811e3ad561ef8de89547ffa991a6911cde35414736172577fa489de5f",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nset nested\n\n1 | obj=new Dynamic { set=Set(new Dynamic { x=throw(\"set nested\") }) }\n                                              ^^^^^^^^^^^^^^^^^^^\nat m2-platform#obj.set.x (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-supplement-strict-set-forced-in-object",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-strict-set-forced-in-object",
    "source": "obj=new Dynamic { set=Set(new Dynamic { x=throw(\"set nested\") }) }\nresult=obj.toString()\n",
    "sourceSha256": "c036b64c3cddd4ee8c96ac600b0204507206492e62a8f67cd1f64175a0733280"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "9c476e9f1ecc9f08210a30372b03124fd8fc0779064e15f1f7bc08d9db2ac6f4"
    },
    "expectedValue": "new D { child { x = 1 }; other { 2 } }",
    "expression": "result",
    "id": "object-string-stage36-supplement-typed-nested-corrected",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-typed-nested-corrected",
    "source": "class C { x: Int=1 }\nclass D { child: C=new C {}; other: Listing<Int> = new { 2 } }\nobj=new D {}\nresult=obj.toString()\n",
    "sourceSha256": "e7f5d619b1ebc1ab5e8655a9446f6a83c6ac2fb8baca30a30cb341e88de4210e"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "4447ec53a54d2ebf9f79ada80da98fd92c59630d95cab997ef2bc4a156cde2fd"
    },
    "expectedValue": "new Dynamic { text = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }",
    "expression": "result",
    "id": "object-string-stage36-supplement-unlimited-single-line",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement-unlimited-single-line",
    "source": "obj=new Dynamic { text=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }\nresult=obj.toString()\n",
    "sourceSha256": "23a6e4d554dc704e2025d9a0595afccca48f7be66441b0477082ba341bf0858a"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "e9cd1a565f8255c17deb38cab0e3fd87e565b9a1d6e92907f0e30d31a902f436"
    },
    "expectedValue": "new Dynamic { x = ?; y = 2 }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-dynamic-undefined-required-property",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-dynamic-undefined-required-property",
    "source": "class C { missing: Int }\nobj=new Dynamic { x=(new C {}).missing; y=2 }\nresult=obj.toString()\n",
    "sourceSha256": "452fe0df48da5b250f2bf6cf9a31c366ecd677cbd35490b63a904223057ad629"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "cc2a1db7db45f1d99c249ca6b9735490a051d97d54c64058cda62047b6328a75"
    },
    "expectedValue": "new Listing { ?; 2 }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-listing-undefined-required-property",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-listing-undefined-required-property",
    "source": "class C { missing: Int }\nobj=new Listing { (new C {}).missing; 2 }\nresult=obj.toString()\n",
    "sourceSha256": "f0c609ba0d17206afe456cfa8894a076c346b4a08d96b9f6367cd22034d6a258"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "2b1ed9dad3b3932c6eded23fefc3d5308f0a7f5dd3b148566f22c8e223f04bb9"
    },
    "expectedValue": "new Mapping { [\"a\"] = ?; [\"b\"] = 2 }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-mapping-undefined-required-property",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-mapping-undefined-required-property",
    "source": "class C { missing: Int }\nobj=new Mapping { [\"a\"]=(new C {}).missing; [\"b\"]=2 }\nresult=obj.toString()\n",
    "sourceSha256": "bb9fc97d8be0f7a942ce9e7912215d7abcca8490685237a328f7fcd1e14ae30e"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a8caa925b0ae6d0f12562d68e727706a078055e063cfe8fb8ad170497ee794a5"
    },
    "expectedValue": "new Dynamic { nested { missing = ? }; y = 2 }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-nested-undefined-required-property",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-nested-undefined-required-property",
    "source": "class C { missing: Int }\nobj=new Dynamic { nested=new C {}; y=2 }\nresult=obj.toString()\n",
    "sourceSha256": "295c0a1ae9d13d025e7ac3233271cc9f7d18316fd5ef82f61fdae764f0978378"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "f4a74b114a39e2a64345a96019057a513ecd8f965ce430ea10e316e8f5a9819c"
    },
    "expectedValue": "new C { missing = ? }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-typed-undefined-expression",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-typed-undefined-expression",
    "source": "class C { missing: Int }\nobj=new C { missing=(new C {}).missing }\nresult=obj.toString()\n",
    "sourceSha256": "d2abd7686259981647a39c5084cc232060a891398851d8354fcc043f6a8ea476"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "e5a951270593795c750dc9734bb06f1aa6c0ccb3b75eac2469a584ed30787d1c",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nafter undefined\n\n2 | obj=new Dynamic { x=(new C {}).missing; y=throw(\"after undefined\") }\n                                              ^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#obj.y (file:///m2-platform.pkl)\n\n3 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-supplement2-undefined-before-later-error",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-undefined-before-later-error",
    "source": "class C { missing: Int }\nobj=new Dynamic { x=(new C {}).missing; y=throw(\"after undefined\") }\nresult=obj.toString()\n",
    "sourceSha256": "fe38f0a49249d80e25b370fa56e9b3419209f8d1d99f84aa201ddcbf33e551a5"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "bba0d78508babfff54b8f421f8cf6e9410aca7ab25bd04ea97c79364642402a8"
    },
    "expectedValue": "new Dynamic { list = List(new C { missing = ? }) }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-undefined-nested-in-strict-list-corrected",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-undefined-nested-in-strict-list-corrected",
    "source": "class C { missing: Int }\nobj=new Dynamic { list=List(new C {}) }\nresult=obj.toString()\n",
    "sourceSha256": "019514dffa71e3a527eda35c44771facacc08c38d0850c28505435cff2872491"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "0c59ea5b320b751ec243a8e0163ae95825ec93742fb29d5af2f1e9cc764462a1"
    },
    "expectedValue": "new Dynamic { map = ? }",
    "expression": "result",
    "id": "object-string-stage36-supplement2-undefined-nested-in-strict-map-key-corrected",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-supplement2-undefined-nested-in-strict-map-key-corrected",
    "source": "class C { missing: Int }\nobj=new Dynamic { map=Map(new C {},1) }\nresult=obj.toString()\n",
    "sourceSha256": "0baafe1966b243862eef78932d9275417f59a2344a16c5697d4f0767097f6b75"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a3240ca85f71f61ec24a058da6c104ab4c2ee31771b5123341bd5dc196e95ac0"
    },
    "expectedValue": "new C {}",
    "expression": "result",
    "id": "object-string-stage36-typed-empty",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-empty",
    "source": "class C {}\nobj=new C {}\nresult=obj.toString()\n",
    "sourceSha256": "e227135714bddb26c05e4228f938c66a4bc1be4cb72a4d4180b2529675712e72"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a0fb75a230840f418a3848c2d5e81aaf2e5102a162d9b03ead71a055a8ceed41"
    },
    "expectedValue": "new C { x = 1 }",
    "expression": "result",
    "id": "object-string-stage36-typed-skip-hidden-local-method",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-skip-hidden-local-method",
    "source": "class C { x=1; hidden secret=throw(\"hidden\"); local private=throw(\"local\"); function f()=throw(\"method\") }\nobj=new C {}\nresult=obj.toString()\n",
    "sourceSha256": "1dd9322ed2bf1a4baa2bfc88be8b1a7b1073aa5c3abf0800f6e3c87a7fd8b72a"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "35f5f0d0f6472d961718ee0f57b0d0057b568d6ea2a9eb963b3a764ac1d57ac3"
    },
    "expectedValue": "new C { x = ?; y = ? }",
    "expression": "result",
    "id": "object-string-stage36-typed-undefined",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-undefined",
    "source": "class C { x: Int; y: String }\nobj=new C {}\nresult=obj.toString()\n",
    "sourceSha256": "a488a28fe0574641cf2de03fe0a4d9500fbe536561a7ab0c4c72306d604835ae"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "edc02ecd3ed7e016ecdebc71dd61c7f50aa096ccfd5625bb021578d8b84a64ed"
    },
    "expectedValue": "new C { x = 1; y = ?; z = 3 }",
    "expression": "result",
    "id": "object-string-stage36-typed-undefined-middle",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-undefined-middle",
    "source": "class C { x: Int=1; y: String; z: Int=3 }\nobj=new C {}\nresult=obj.toString()\n",
    "sourceSha256": "8a42550ab24f73b87a1611d81fcb1be39ecec9fdb75556f6c957ae72fe2e2955"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "12d899e725ff7266c891238039827d0b0dace64b092ebe7903511cb996f23c92"
    },
    "expectedValue": "new C { x = 1; y = \"two\" }",
    "expression": "result",
    "id": "object-string-stage36-typed-values",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-values",
    "source": "class C { x: Int=1; y: String=\"two\" }\nobj=new C {}\nresult=obj.toString()\n",
    "sourceSha256": "ba0a5dde60650a0a27f7bb63baf506e96d6b2376ec99557ac96227d25e00f91b"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "8ccafbfda7c2711d6e26cc00e2f8f1c87e8fc3080dbe4da8274281b57e7c33d9",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot find property `missing`.\n\n1 | obj = new Dynamic { x=missing; y=2 }\n                          ^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\nDid you mean any of the following?\nListing\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-unknown-name-not-undefined",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-unknown-name-not-undefined",
    "source": "obj = new Dynamic { x=missing; y=2 }\nresult=obj.toString()\n",
    "sourceSha256": "ac56a23d036391280a8ba9d81abfc37aa687f8d68e42df2a49b4ced94f0820f3"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "df177f14eb02e363b3acbd156426a9a6573eed3a964cf29c16061af93f7e4910"
    },
    "expectedValue": "new C { x = 5 }",
    "expression": "result",
    "id": "object-string-stage36-typed-undefined-parent-fallback",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-undefined-parent-fallback",
    "source": "open class B { x: Int=5 }\nclass C extends B { x: Int }\nresult=new C {}.toString()\n",
    "sourceSha256": "42a8463bf2a1d486f6bbe4232c8e997069748a4a0fd027982d7cce36d5c37c3a"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "df177f14eb02e363b3acbd156426a9a6573eed3a964cf29c16061af93f7e4910"
    },
    "expectedValue": "new C { x = 5 }",
    "expression": "result",
    "id": "object-string-stage36-typed-undefined-expression-parent-fallback",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-typed-undefined-expression-parent-fallback",
    "source": "class Required { missing: Int }\nopen class B { x: Int=5 }\nclass C extends B { x=(new Required {}).missing }\nresult=new C {}.toString()\n",
    "sourceSha256": "f84392873a95dc064c34692dddbdf029b1a44288510cb9a612806c42cb0d5670"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "36f68b08ec34be3c3893ca311eb605edfd01bd40b457db80f8a0b0c7b69c5d52"
    },
    "expectedValue": "new Dynamic { x = 5 }",
    "expression": "result",
    "id": "object-string-stage36-dynamic-undefined-parent-fallback",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-dynamic-undefined-parent-fallback",
    "source": "class Required { missing: Int }\nparent=new Dynamic { x=5 }\nobj=(parent) { x=(new Required {}).missing }\nresult=obj.toString()\n",
    "sourceSha256": "8dc5cfeca874152a83e1b344bc7065f006a4d7a4a0aaaaf2c5da5af3267a69f1"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "6cdfd271da635d491e37a2b4a1044b306e6e9e039aeadee95bb355efadf8cb33"
    },
    "expectedValue": "custom",
    "expression": "result",
    "id": "object-string-stage36-class-to-string-override",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-class-to-string-override",
    "source": "class C { bad=throw(\"unforced\"); function toString(): String=\"custom\" }\nresult=new C {}.toString()\n",
    "sourceSha256": "978e5d86a662d049d7fb752c519210d6b18bacef6b528f5bc8f5ed24c359eea5"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "8532126e191d8c8678b04d2e2d7a31e3993ac1c54e2350fc12efb0cd0aaae01b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nforced\n\n1 | obj=new Dynamic { local function toString()=\"local\"; bad=throw(\"forced\") }\n                                                             ^^^^^^^^^^^^^^^\nat m2-platform#obj.bad (file:///m2-platform.pkl)\n\n2 | result=obj.toString()\n           ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-local-to-string-method",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-local-to-string-method",
    "source": "obj=new Dynamic { local function toString()=\"local\"; bad=throw(\"forced\") }\nresult=obj.toString()\n",
    "sourceSha256": "6acb65bb658ed6d89a40d909bc9e03eec523e6b01afc3efcb57706d6c69dec71"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "1dfc5c4adbea917e94f1d4ef6bc3274eca8dccfeffc249ceefc33497af4cde7e",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot call method `toString` from here because it is not `const`.\n\n2 | const function render(): String=module.toString()\n                                    ^^^^^^^^^^^^^^^^^\nat m2-platform#render (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to method `toString`\n2. Self-import this module, and reference this method from the import.\n\n3 | hidden result=render()\n                  ^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-const-module-to-string",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-const-module-to-string",
    "source": "x=1\nconst function render(): String=module.toString()\nhidden result=render()\n",
    "sourceSha256": "a92c386ae1f33f064324547f8f323d10447d24407a38699b575dc219ab1cf633"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "4b0bd2afa1673e7906900d82e5546cba046d113fc6d6628abb2c1d39242edf20",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot call method `toString` from here because it is not `const`.\n\n1 | @Deprecated { message=module.toString() }\n                          ^^^^^^^^^^^^^^^^^\nat m2-platform#f.message (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to method `toString`\n2. Self-import this module, and reference this method from the import.\n",
    "expression": "result",
    "id": "object-string-stage36-annotation-module-to-string",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-annotation-module-to-string",
    "source": "@Deprecated { message=module.toString() }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "ff202a5e0a1bea86676c1569d11e693a6e7e04c1e985fb181cc1ff7f8d116155"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df"
    },
    "expectedValue": "ok",
    "expression": "result",
    "id": "object-string-stage36-annotation-this-to-string",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-annotation-this-to-string",
    "source": "@Deprecated { message=this.toString() }\nfunction f()=1\nresult=\"ok\"\n",
    "sourceSha256": "e2e1be81a5e6e49ba78e1c99404cb68cb1eec77e0778ad1994c813a0c4ec28ff"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "02ce04240c0b52740b8b68612367efe8b4e4ace5364ca26e400a456d315559db"
    },
    "expectedValue": "List(true, false)",
    "expression": "result",
    "id": "object-string-stage36-utf16-rendered-value-identity",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-utf16-rendered-value-identity",
    "source": "obj=new Dynamic { x=\"\\u{d800}\" }\nexpected=\"new Dynamic { x = \\\"\\u{d800}\\\" }\"\nreplacement=\"new Dynamic { x = \\\"?\\\" }\"\nresult=List((obj.toString()==expected),(obj.toString()==replacement)).toString()\n",
    "sourceSha256": "c958efe70a4547e1d9258c49909806baae679fdefda09865520f7a8a99e3229f"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "4818392c3cac5a6a2f9a853e76e7d905651107ad3000459d3c6786e44827a85a"
    },
    "expectedValue": "new Dynamic { minimum = -9223372036854775808; zero = -0.0; infinity = Infinity; nan = NaN }",
    "expression": "result",
    "id": "object-string-stage36-numeric-values",
    "oracleCasePath": "target/m2/stage36-selected-oracle-01/cases/object-string-stage36-numeric-values",
    "source": "obj=new Dynamic { minimum=-9223372036854775808; zero=-0.0; infinity=1.0/0.0; nan=0.0/0.0 }\nresult=obj.toString()\n",
    "sourceSha256": "0868ac2957f7e5f7d27df71a1f4e23baf30a5309d0a01bf7a4df45c62e08a5d1"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "b21c98a2704c947711dac7eccfb86d4e2a6e7e2b8e1fe35d39b0f70f3da8477a",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nbase\n\n1 | open class B { x=throw(\"base\") }\n                     ^^^^^^^^^^^^^\nat m2-platform#B.x (file:///m2-platform.pkl)\n\n3 | result=new C {}.toString()\n           ^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-owner-hidden-base-error",
    "oracleCasePath": "target/m2/stage36-owner-hidden-selected-oracle-01/cases/object-string-stage36-owner-hidden-base-error",
    "source": "open class B { x=throw(\"base\") }\nclass C extends B { hidden x=2 }\nresult=new C {}.toString()\n",
    "sourceSha256": "5e3c6692e70a6ac5719c105cdf5538b04a1e0408267c9515263ae6270d976ed6"
  },
  {
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stderr": "b21c98a2704c947711dac7eccfb86d4e2a6e7e2b8e1fe35d39b0f70f3da8477a",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nbase\n\n1 | open class B { x=throw(\"base\") }\n                     ^^^^^^^^^^^^^\nat m2-platform#B.x (file:///m2-platform.pkl)\n\n3 | result=new C {}.toString()\n           ^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
    "expression": "result",
    "id": "object-string-stage36-owner-hidden-child-error-base-error",
    "oracleCasePath": "target/m2/stage36-owner-hidden-selected-oracle-01/cases/object-string-stage36-owner-hidden-child-error-base-error",
    "source": "open class B { x=throw(\"base\") }\nclass C extends B { hidden x=throw(\"child\") }\nresult=new C {}.toString()\n",
    "sourceSha256": "a2c59d66114af2a4a321501e0c478707c9c00a9105b66e9456a5e2bb2bf9272c"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b"
    },
    "expectedValue": "1",
    "expression": "result",
    "id": "object-string-stage36-review-direct-to-string-explicit-int-return",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-direct-to-string-explicit-int-return",
    "source": "class C { function toString(): Int=1 }\nresult=new C {}.toString()\n",
    "sourceSha256": "d17aaab6f3c1afafd47ea01f0dd227c98113de515c16f18b05f4aad7220bec0f"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b"
    },
    "expectedValue": "1",
    "expression": "result",
    "id": "object-string-stage36-review-direct-to-string-int-return",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-direct-to-string-int-return",
    "source": "class C { function toString()=1 }\nresult=new C {}.toString()\n",
    "sourceSha256": "50774d99d93100d839c2c37eec4ba29a2c2577b6c8eca0085531e43c736f48d8"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "efd40fce9676c1637cdff70c1cd35138c08f255db1c18a19d8648e8a2d1f7eaa"
    },
    "expectedValue": "new Dynamic { x = 1 }",
    "expression": "result",
    "id": "object-string-stage36-review-dynamic-hidden-default-control",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-dynamic-hidden-default-control",
    "source": "obj=new Dynamic { default=throw(\"hidden\"); x=1 }\nresult=obj.toString()\n",
    "sourceSha256": "c79c3ed683f255452fae1a560fde04a1551b29603133b18076b110f8000dfc94"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "0aff4782ef3393fcfef263b967fef8abf6c6619a2f70893a7a7a7fe53cf66785"
    },
    "expectedValue": "new Mapping { [new Dynamic { x = 1 }] = \"child\" }",
    "expression": "result",
    "id": "object-string-stage36-review-inherited-equal-object-keys",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-inherited-equal-object-keys",
    "source": "k1=new Dynamic { x=1 }\nk2=new Dynamic { x=1 }\nbase=new Mapping { [k1]=\"base\" }\nobj=(base) { [k2]=\"child\" }\nresult=obj.toString()\n",
    "sourceSha256": "f0c34c46138da8961bb2c456a69998bd4c6a07d115e26a458b49564abcf06ef5"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "0aff4782ef3393fcfef263b967fef8abf6c6619a2f70893a7a7a7fe53cf66785"
    },
    "expectedValue": "new Mapping { [new Dynamic { x = 1 }] = \"child\" }",
    "expression": "result",
    "id": "object-string-stage36-review-inherited-equal-object-keys-cached-control",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-inherited-equal-object-keys-cached-control",
    "source": "k1=new Dynamic { x=1 }\nk2=new Dynamic { x=1 }\nbase=new Mapping { [k1]=\"base\" }\nobj=(base) { [k2]=\"child\" }\nresult=let (ignore=obj[k1]) obj.toString()\n",
    "sourceSha256": "22bccd51a9871bf9d4dd0195868b82fb4bda28a394e3c8b4a81bcd55a05820ca"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "844eb533ceaf7136c4f7808f265468955a042220e937d5520280c13c884bb69e"
    },
    "expectedValue": "new Mapping { [new Dynamic { x = 1 }] = \"base\"; [new Dynamic { y = 0 }] = \"child\" }",
    "expression": "result",
    "id": "object-string-stage36-review-inherited-unequal-colliding-object-keys",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-inherited-unequal-colliding-object-keys",
    "source": "k1=new Dynamic { x=1 }\nk2=new Dynamic { y=0 }\nbase=new Mapping { [k1]=\"base\" }\nobj=(base) { [k2]=\"child\" }\nresult=obj.toString()\n",
    "sourceSha256": "0ce0ad407c1804e44b7f6d31b71c0998c10a98138b115fe67abd3c13ab14b365"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "6cdfd271da635d491e37a2b4a1044b306e6e9e039aeadee95bb355efadf8cb33"
    },
    "expectedValue": "custom",
    "expression": "result",
    "id": "object-string-stage36-review-interpolation-user-to-string-override",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-interpolation-user-to-string-override",
    "source": "class C { bad=throw(\"unforced\"); function toString(): String=\"custom\" }\nresult=\"\\(new C {})\"\n",
    "sourceSha256": "28b52b8d1f4828b91fd5fc24022d33dac5fc0caaa4e24b62577a0b4d55cc0679"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "c78e5f31415cd049f8ced1814441572314ada9a1d0651c35a468b3aca9d359b7"
    },
    "expectedValue": "new Listing { 1 }",
    "expression": "result",
    "id": "object-string-stage36-review-listing-hidden-default-control",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-listing-hidden-default-control",
    "source": "obj=new Listing { default=throw(\"hidden\"); 1 }\nresult=obj.toString()\n",
    "sourceSha256": "3b36eda9218f36e52e7e1d77f26d8abf68c6a74081183b3ee5aeedf223e42d90"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "3252e5bb113c83a0130569b96f13290fbb3edc2db173b196c31389329ac67bd8"
    },
    "expectedValue": "new Mapping { [\"x\"] = 1 }",
    "expression": "result",
    "id": "object-string-stage36-review-mapping-hidden-default",
    "oracleCasePath": "target/m2/stage36-review-selected-oracle-01/cases/object-string-stage36-review-mapping-hidden-default",
    "source": "obj=new Mapping { default=throw(\"hidden\"); [\"x\"]=1 }\nresult=obj.toString()\n",
    "sourceSha256": "2566c101e449c9484e5efd471ca9cdaaec3ae9380562fa6a5c4e945a03c06c38"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "c14c7de5acc0ff5fedc757d2afc7f31d59d051b567d39e56d4cbeab804e65454"
    },
    "expectedValue": "new Mapping { [new Dynamic { [\"x\"] = 1 }] = \"base\"; [new Dynamic { [\"y\"] = 0 }] = \"child\" }",
    "expression": "result",
    "id": "object-string-stage36-review-string-entry-hash-collision",
    "oracleCasePath": "target/m2/stage36-review-collision-selected-oracle-01/cases/object-string-stage36-review-string-entry-hash-collision",
    "source": "k1=new Dynamic { [\"x\"]=1 }\nk2=new Dynamic { [\"y\"]=0 }\nbase=new Mapping { [k1]=\"base\" }\nobj=(base) { [k2]=\"child\" }\nresult=obj.toString()\n",
    "sourceSha256": "85674c14de777df824480a5225644758287e44d55853eed993e9099bcf327b7b"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a0fb75a230840f418a3848c2d5e81aaf2e5102a162d9b03ead71a055a8ceed41"
    },
    "expectedValue": "new C { x = 1 }",
    "expression": "result",
    "id": "object-string-stage36-review-modifier-import-bit",
    "oracleCasePath": "target/m2/stage36-review-modifier-selected-oracle-01/cases/object-string-stage36-review-modifier-import-bit",
    "source": "class C { const const x=1 }\nresult=new C {}.toString()\n",
    "sourceSha256": "d86913c8bb16538dbe2e4f18797b2918bf25dcd85bdbbf1271ee24db2f5e78fa"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a3240ca85f71f61ec24a058da6c104ab4c2ee31771b5123341bd5dc196e95ac0"
    },
    "expectedValue": "new C {}",
    "expression": "result",
    "id": "object-string-stage36-review-modifier-class-bit",
    "oracleCasePath": "target/m2/stage36-review-modifier-selected-oracle-01/cases/object-string-stage36-review-modifier-class-bit",
    "source": "class C { const const const const x=1 }\nresult=new C {}.toString()\n",
    "sourceSha256": "cb23613abf4364c37f783ac42a7fb82a48cb2526b09930aa09e58b1202c3bfb2"
  },
  {
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stdout": "a3240ca85f71f61ec24a058da6c104ab4c2ee31771b5123341bd5dc196e95ac0"
    },
    "expectedValue": "new C {}",
    "expression": "result",
    "id": "object-string-stage36-review-modifier-type-alias-bit",
    "oracleCasePath": "target/m2/stage36-review-modifier-selected-oracle-01/cases/object-string-stage36-review-modifier-type-alias-bit",
    "source": "class C { const const const const const const const const x=1 }\nresult=new C {}.toString()\n",
    "sourceSha256": "1bd7ac153878a082d313a3f454cc17de6eb21246b6623ec73a02f03f3cc447ab"
  }
];

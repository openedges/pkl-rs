// Complete fresh pinned Java 0.32.0 bytes at the identical platform URI.
// Numeric lowering changes value-frame spans while authored AST spans remain intact.
export const numericValueFrameContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "oracle": {
    "acquisition": "target/m2/stage31-platform-oracle-01",
    "acquisitionReportSha256": "15874ae91c48da0d9d56671de9a4508b24430ce0f92704d094ec052d728dc791",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d"
  },
  "excludedExplorationCases": {
    "object-public-negative-float": "Existing invalid object annotation diagnostic frame gap.",
    "module-alias-double-minint": "Existing runtime overflow diagnostic frame gap."
  }
};

export const numericValueFrameCases = [
  {
    "id": "module-alias-negative-int",
    "oracleCaseId": "module-alias-negative-int",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "29a7f6c49fe752d7dea9a06e4fa9ead4ed56a14a0c29b632215723d752702930",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9c897199467d8b54d5be6d98497b1ff67446ac3c2a321f57ad05d764f7ee3019"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -1\n                    ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-space",
    "oracleCaseId": "module-alias-negative-space",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = - 1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "68384ac048c0e01234ea76d44e9c4f3c4a33c2cd43142468fc44ad12149da89b",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "28eb77b9779acf7fffafcec5f1ac1ef3b715377fa1c3b87322bef27ad7283263"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = - 1\n                     ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-comment",
    "oracleCaseId": "module-alias-negative-comment",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = - /* gap */ 1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "ab24112cd1a67e41273497792bdd79dac524f4889ceada01326722a959f5fd13",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "42024afb1b3ec53532306b5bbe978248babe1e5540042bb72b832aadd3aed269"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = - /* gap */ 1\n                               ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-multiline",
    "oracleCaseId": "module-alias-negative-multiline",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -\n  1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "789cf136bf53a4ab9859c5ff4d59cb23dd7673af65c9c1376b98c724311cc3a9",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a6968ce5780ebb199a73745d314457d975329418929a6af9cafbd254f9b87559"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | 1\n    ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n4 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-hex",
    "oracleCaseId": "module-alias-negative-hex",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -0x2a\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "6ac4c7dab8872e75316fadc03609ca0a865b52ccdc79da91c1c7ad7289f248d4",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ebc2d74ad89123bfa63e0310fc77a8ce548ecdbbeabbb0dfa5950fff040d02e1"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -42\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -0x2a\n                    ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-binary",
    "oracleCaseId": "module-alias-negative-binary",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -0b101010\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "ebc07abd18e9ae2f2b5781b258b9c62349594ab1831e71ef1b5a2365f8f3b5b1",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e06accda4ab06e72abb751fc7b426dff9dc29dcaf10623d8cbcd7fe595997fc7"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -42\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -0b101010\n                    ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-octal",
    "oracleCaseId": "module-alias-negative-octal",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -0o52\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "37a449fc4dbc63e65be5a7e545588c9e2177843b7328bd1492cda59f78493623",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c00eaa6c8359780f835d098f2d690d3b1df27ad079b77c2153195c9ebd041bd0"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -42\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -0o52\n                    ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-float",
    "oracleCaseId": "module-alias-negative-float",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -1.25\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "897552865213a99fa9dd18f9014e093e3bd44cd72d6f049aa1efa1a2fd122e1b",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ac0edb942ff0a708544ae7ab7f17d0fe023c1f418db962fb84c13eef54c15516"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Float`.\nValue: -1.25\n\n1 | typealias I = Int(this > 2000)\n                  ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -1.25\n                    ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-float-zero",
    "oracleCaseId": "module-alias-negative-float-zero",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -0.0\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "b3176c60728a32fa7e465f669cea27d16577eb61b8fd7d6fd018632bde538a6c",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "87b7b3a180ac1eaf5e8c9939a856d71ec9ec12f0de1ca5c1352a24fe8012af5e"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Float`.\nValue: -0.0\n\n1 | typealias I = Int(this > 2000)\n                  ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -0.0\n                    ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-grouped-negative",
    "oracleCaseId": "module-alias-grouped-negative",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = (-1)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "24f7e9f8895fd70e1a67de46d44d56c7fc0c5992184bfef73c6b895bfda1c16f",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "94c96a4e944d4e1d4b936654a8c687d80c9447871ecb8c648e683b143ff9bf39"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = (-1)\n                     ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-nested-grouped-negative",
    "oracleCaseId": "module-alias-nested-grouped-negative",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = (( -1 ))\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "1dd6eec0ba82e93fd2c6707652efdf2e978bfb086f07581bda0f36722fcf192d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5afa1a52fbd3f93ba25bd94c81525d0b2cbd8ce057d33427f81685c5ac6d2c08"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = (( -1 ))\n                       ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-grouped",
    "oracleCaseId": "module-alias-negative-grouped",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -(1)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "c576948c7464dddea02147ff7415014324e1fd393dc2416330c6ac9db86ff0dc",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "444819dbaae96274e5fd0da6cf7195d863b0d6cde3888fa1cfbf917fe18e805c"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -(1)\n                   ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-grouped-float",
    "oracleCaseId": "module-alias-negative-grouped-float",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -(1.25)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "4f8e44133248cf3ba290beeab176ca5403edd12392ba80dcb2940df03a3f7239",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f08e751f0a562ce89f44e64a7cef604754f55e330904e85bef9827490c137d10"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Float`.\nValue: -1.25\n\n1 | typealias I = Int(this > 2000)\n                  ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -(1.25)\n                   ^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-double-negative",
    "oracleCaseId": "module-alias-double-negative",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = --1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "3cdb643a67083d85b3258bce903bbade1bed0ea4145d3c9e766deb54c1ddd8fe",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "df33f6701cc18440aa0d3af2410602f4e24752c62a5fe824712a47a1c718ff17"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: 1\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = --1\n                   ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-negative-minint",
    "oracleCaseId": "module-alias-negative-minint",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -9223372036854775808\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "17b80e792642769197cbaa43eb5d5a4ea2d2e7d2d581194310052733115a6da0",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d61d20e6cd3cbb3b8e0705f539adb9894ab9c9f893addb7e1f593f4cdac4695f"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 2000` violated.\nValue: -9223372036854775808\n\n1 | typealias I = Int(this > 2000)\n                      ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | local bad: I = -9223372036854775808\n                    ^^^^^^^^^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-alias-grouped-overflow",
    "oracleCaseId": "module-alias-grouped-overflow",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -(9223372036854775808)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "6d8446626bf6364ba8ef781939b298cca895786eeb83f16a42bbe00c14ae4e34",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f218da3b841cd3e92ed2983f58ec5557d8ef1a22e88cc8bfe25e7c87f15f5d38"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInt literal `9223372036854775808` is too large.\n\n2 | local bad: I = -(9223372036854775808)\n                     ^^^^^^^^^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "module-alias-negative-overflow",
    "oracleCaseId": "module-alias-negative-overflow",
    "source": "typealias I = Int(this > 2000)\nlocal bad: I = -9223372036854775809\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "2191f0572439967986fe171e4c3a64b4c0c37412cc507ca68c2dd96637de9102",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d4be9c4c633a403de7033da2b85a9963dbced75dff12c5673011935deada2b2b"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInt literal `9223372036854775809` is too large.\n\n2 | local bad: I = -9223372036854775809\n                    ^^^^^^^^^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "class-local-negative-int",
    "oracleCaseId": "class-local-negative-int",
    "source": "typealias I = Int(this > 0)\nclass C { local bad: I = -1; result = bad }\nresult = new C {}.result\n",
    "expression": "result",
    "sourceSha256": "2ff018671a9a8e94d5f389ad0c9cea5eef9ab72cda5a529afb2e3afae8905943",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "47fccd401da50f90a863d31d78df753508083593b08e8319cad4e3f3dbe73286"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | class C { local bad: I = -1; result = bad }\n                              ^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | class C { local bad: I = -1; result = bad }\n                                          ^^^\nat m2-platform#C.result (file:///m2-platform.pkl)\n\n3 | result = new C {}.result\n             ^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "class-public-grouped-negative",
    "oracleCaseId": "class-public-grouped-negative",
    "source": "typealias I = Int(this > 0)\nclass C { bad: I = (-1) }\nresult = new C {}.bad\n",
    "expression": "result",
    "sourceSha256": "eaf086b471ad5317574c247e309dfc8570002fe5f5a343162f0445df973696cd",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "0994e7b714523024b00ffa88d93d19fe689bcb4fbf3e6d3248f2fc160235f64c"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | class C { bad: I = (-1) }\n                       ^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n3 | result = new C {}.bad\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "object-local-negative-int",
    "oracleCaseId": "object-local-negative-int",
    "source": "typealias I = Int(this > 0)\nx = new { local bad: I = -1; result = bad }\nresult = x.result\n",
    "expression": "result",
    "sourceSha256": "463f0609b97b13607da536056afb3f7c95021e5cf0add3925226c70b252db29e",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "958f63465c2ab5ed15d04c422421ae12a53361dec2afed74d3715b95d1f2952f"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n\n2 | x = new { local bad: I = -1; result = bad }\n                                          ^^^\nat m2-platform#x.result (file:///m2-platform.pkl)\n\n3 | result = x.result\n             ^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-direct-negative-int",
    "oracleCaseId": "module-direct-negative-int",
    "source": "local bad: Int(this > 0) = -1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "2cdc7ad002f63effc926432eb423a3cd09eb51f1b162d6acd41ed3320872521f",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "111e6a694f44071db6053181200e147ddb3a5bebee8fab275162dcbb44ee1525"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | local bad: Int(this > 0) = -1\n                   ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: Int(this > 0) = -1\n                                ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-mismatch-negative-int",
    "oracleCaseId": "module-mismatch-negative-int",
    "source": "local bad: String = -1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "9d97eaa2f744139831d323bfe033050e2f424ee26dc5ac031246dede0530ed49",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "870bfa0da7153e29e6c3f0307d154dc0fd096fe9aabe9cb288ac0dafc88d1bee"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `String`, but got type `Int`.\nValue: -1\n\n1 | local bad: String = -1\n               ^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: String = -1\n                         ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "argument-alias-grouped-negative",
    "oracleCaseId": "argument-alias-grouped-negative",
    "source": "typealias I = Int(this > 0)\nfunction bad(x: I) = x\nresult = bad((-1))\n",
    "expression": "result",
    "sourceSha256": "e681cd3c8ea816af159ff746c84d4cb3c578d1aeb88d20717d97e528fa56140e",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5ad177ade9136bfe108ac85fe5f7771fe0d1389a4bc2ba1d7edf26a6aee807f4"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad((-1))\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "argument-direct-negative-float",
    "oracleCaseId": "argument-direct-negative-float",
    "source": "function bad(x: Int(this > 0)) = x\nresult = bad(-1.25)\n",
    "expression": "result",
    "sourceSha256": "c4b59e51be1d799faf84fff936938695b57dc1290a5955091a0088af7154c35b",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8428cec3c598464b8fe638d3d720218d41cb8925e9e5fbab6449b8d3585da584"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Float`.\nValue: -1.25\n\n1 | function bad(x: Int(this > 0)) = x\n                    ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad(-1.25)\n             ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "return-alias-grouped-negative",
    "oracleCaseId": "return-alias-grouped-negative",
    "source": "typealias I = Int(this > 0)\nfunction bad(): I = (-1)\nresult = bad()\n",
    "expression": "result",
    "sourceSha256": "295b006cbfc71dc1ca9dda4089dff5079f3f1be2ed52838a592ed46e15f8f99b",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ace26ccea7feebc73b159459c92539063a6cd58f04010f1670c08e49644eda40"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad()\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "return-direct-negative-int",
    "oracleCaseId": "return-direct-negative-int",
    "source": "function bad(): Int(this > 0) = -1\nresult = bad()\n",
    "expression": "result",
    "sourceSha256": "b9e046ac4f83c46306f4e1e82a778c5299029a0b244f544c4aa005f518abb0b9",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6e72cd361ff62c7f9f54dcf6ab2cad2b645201340c7dad7ffa9c3d00638fa378"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | function bad(): Int(this > 0) = -1\n                        ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad()\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "cast-grouped-negative",
    "oracleCaseId": "cast-grouped-negative",
    "source": "result = (-1) as Int(this > 0)\n",
    "expression": "result",
    "sourceSha256": "f4dfebb8e02805b43fefd7309b23c95ba29508d44905884e62423fa2abe25d39",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d430fd16bea7dc4b5dcfdcb500efc37a583a2eceba5572171a8d5eedb811c36d"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | result = (-1) as Int(this > 0)\n                         ^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "first-error-negative-int",
    "oracleCaseId": "first-error-negative-int",
    "source": "local bad: Int(this > 0) = -1\nlocal other = missing\nresult = bad + other\n",
    "expression": "result",
    "sourceSha256": "e188aa78f285fb6ed6f9520edfba9dd3ea0342f873667f7e36571bd95d3a6054",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4ebd67d90ffec17660c3b41d559c1f276ea87918f11576201d8361e7025a39d8"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | local bad: Int(this > 0) = -1\n                   ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: Int(this > 0) = -1\n                                ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad + other\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "success-minint",
    "oracleCaseId": "success-minint",
    "source": "result = -9223372036854775808\n",
    "expression": "result",
    "sourceSha256": "517f765ce9c8daec2df3bea0b9bc32245b424f827650bf1c031788b5c4ee3f4e",
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "85386477f3af47e4a0b308ee3b3a688df16e8b2228105dd7d4dcd42a9807cb78",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "-9223372036854775808"
  },
  {
    "id": "success-negative-zero",
    "oracleCaseId": "success-negative-zero",
    "source": "result = 1.0 / -0.0\n",
    "expression": "result",
    "sourceSha256": "94fe05ce8061cb0a1d1efeae8b82dffa636847b58bd1685b9118abdb9e9419aa",
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "c64ddf11bcd45660f0cf66dd0c22d2b4570ef3d3fc6527a9a6f6c722aefa3c39",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "-Infinity"
  },
  {
    "id": "success-negative-grouped",
    "oracleCaseId": "success-negative-grouped",
    "source": "result = -(1)\n",
    "expression": "result",
    "sourceSha256": "33ba7785e08a1a49a824ff7620e617f7a5b9e223fecee478dd31df5f97fe85fb",
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "1bad6b8cf97131fceab8543e81f7757195fbb1d36b376ee994ad1cf17699c464",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "-1"
  },
  {
    "id": "success-grouped-negative",
    "oracleCaseId": "success-grouped-negative",
    "source": "result = (-1)\n",
    "expression": "result",
    "sourceSha256": "a443a9f13e480d2e940638eef6b82aaefe9edaa004623e56519e31e5a5cb4458",
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "1bad6b8cf97131fceab8543e81f7757195fbb1d36b376ee994ad1cf17699c464",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "-1"
  },
  {
    "id": "success-double-negative",
    "oracleCaseId": "success-double-negative",
    "source": "result = --1\n",
    "expression": "result",
    "sourceSha256": "988e293d84247088875c34fe5250b20a9e8b308d4f12c95ded3250f65058a826",
    "captureSha256": {
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "1"
  },
  {
    "id": "class-public-negative-int",
    "oracleCaseId": "class-public-negative-int",
    "source": "typealias I = Int(this > 0)\nclass C { bad: I = -1 }\nresult = new C {}.bad\n",
    "expression": "result",
    "sourceSha256": "763f090583918677bac3893d54f30efbfdb57708f3cc0fb2eac8e387394c66f3",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1dd5c13fb14c8caf5bc35541c611f6a3c82088fa465b308ed5327d63bcc797a3"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-01",
    "oracleReportSha256": "8b5a60cebdf75c914ec4f591b33adb71a2b1ad0f6ee6773609273c29fbdee0b0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | class C { bad: I = -1 }\n                       ^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n3 | result = new C {}.bad\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "class-public-negative-float",
    "oracleCaseId": "class-public-negative-float",
    "source": "typealias I = Int(this > 0)\nclass C { bad: I = -1.25 }\nresult = new C {}.bad\n",
    "expression": "result",
    "sourceSha256": "1d5c3aee9854129e34d385bf03e89e96dfbe40d524807c0e3805fc01088ee34d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1b2a8ad3e4395a3f6bfa36b3368abcf6114ee3b5a5ad0f60826222f6c63804cd"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-01",
    "oracleReportSha256": "8b5a60cebdf75c914ec4f591b33adb71a2b1ad0f6ee6773609273c29fbdee0b0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Float`.\nValue: -1.25\n\n1 | typealias I = Int(this > 0)\n                  ^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | class C { bad: I = -1.25 }\n                       ^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n3 | result = new C {}.bad\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-public-alias-negative",
    "oracleCaseId": "module-public-alias-negative",
    "source": "typealias I = Int(this > 0)\nbad: I = -1\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "25240485629f8ef27de1570e560f0814eff0cefd9b3250a3e6f26f8282d06676",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9e05cbb897e7194d5407c9222fd1264ee51322c5380de93bbb8f5b7b7c2c2344"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | bad: I = -1\n             ^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "module-public-alias-grouped",
    "oracleCaseId": "module-public-alias-grouped",
    "source": "typealias I = Int(this > 0)\nbad: I = (-1)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "441872079d2f7298dbc9a40d24880a2070476bed86c0eedd3a22130738d8aa09",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "cb926efd3ed7ffc138556110b4e8e159026497ea5213fb96a9ebf8f6e138578b"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | bad: I = (-1)\n             ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n3 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "class-amendment-grouped",
    "oracleCaseId": "class-amendment-grouped",
    "source": "typealias I = Int(this > 0)\nclass C { bad: I = 1 }\nx = new C { bad = (-1) }\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "41703d61cac36933871cc5dd5461c665b26a4caaef2218d4a3390256db17fbe6",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "0779f38d56fef95ae3cece6ec957b80d7e7c3af8b6268a7053baf190d3ba331c"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | typealias I = Int(this > 0)\n                      ^^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n3 | x = new C { bad = (-1) }\n                      ^^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n\n4 | result = x.bad\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "mapping-direct-negative",
    "oracleCaseId": "mapping-direct-negative",
    "source": "x: Mapping<String,Int(this > 0)> = new { [\"a\"] = -1 }\nresult = x[\"a\"]\n",
    "expression": "result",
    "sourceSha256": "afb2bc5c6bd2adb4b18426b594e4f4a550abff5e3a1dbc8c43029ff563183e59",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "31f2029c3dd3cdacba8804a015c8866c7b8d3660e5b7d9390bcf596e114dd26c"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | x: Mapping<String,Int(this > 0)> = new { [\"a\"] = -1 }\n                          ^^^^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n\n1 | x: Mapping<String,Int(this > 0)> = new { [\"a\"] = -1 }\n                                                     ^^\nat m2-platform#x[\"a\"] (file:///m2-platform.pkl)\n\n2 | result = x[\"a\"]\n             ^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "mapping-direct-grouped",
    "oracleCaseId": "mapping-direct-grouped",
    "source": "x: Mapping<String,Int(this > 0)> = new { [\"a\"] = (-1) }\nresult = x[\"a\"]\n",
    "expression": "result",
    "sourceSha256": "6ca51102e0c7822e854c1c240c09af51e7650b148625a99591c78534f5a9736d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4fb90d6ee6af086f53ba04326ae804a62df0476d1f06545befb66230200e8f49"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | x: Mapping<String,Int(this > 0)> = new { [\"a\"] = (-1) }\n                          ^^^^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n\n1 | x: Mapping<String,Int(this > 0)> = new { [\"a\"] = (-1) }\n                                                     ^^^^\nat m2-platform#x[\"a\"] (file:///m2-platform.pkl)\n\n2 | result = x[\"a\"]\n             ^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "listing-direct-negative",
    "oracleCaseId": "listing-direct-negative",
    "source": "x: Listing<Int(this > 0)> = new { -1 }\nresult = x[0]\n",
    "expression": "result",
    "sourceSha256": "5b4e031bbdd1c1086e132f33290d8872e776f2515158c95e0ba073fda5e4b084",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "027c63b5bdb49241160b0e6b508f4954cbc1cdf5c25825638487d01edf7b26bf"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | x: Listing<Int(this > 0)> = new { -1 }\n                   ^^^^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n\n1 | x: Listing<Int(this > 0)> = new { -1 }\n                                      ^^\nat m2-platform#x[#1] (file:///m2-platform.pkl)\n\n2 | result = x[0]\n             ^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "listing-direct-grouped",
    "oracleCaseId": "listing-direct-grouped",
    "source": "x: Listing<Int(this > 0)> = new { (-1) }\nresult = x[0]\n",
    "expression": "result",
    "sourceSha256": "6e05baabf15c3f0213f9d956ec4f3584ac379d988774fac60d2f88f9542b2a14",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1ce5ecec123c7c38fc0906fffb6e3589567df69737002ed93713538c832213b7"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `this > 0` violated.\nValue: -1\n\n1 | x: Listing<Int(this > 0)> = new { (-1) }\n                   ^^^^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n\n1 | x: Listing<Int(this > 0)> = new { (-1) }\n                                         ^\nat m2-platform#x[#1] (file:///m2-platform.pkl)\n\n2 | result = x[0]\n             ^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "local-string",
    "oracleCaseId": "local-string",
    "source": "local bad: Int = (\"wrong\")\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "e0faf6b529d20aefec6356de3e15a6365bdb8d9ea447801ba1c5f75f8501cfa7",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8a1a36d440e259c02aa81e6ecb4e2c7374a818dd5bb89819939f9873e340b149"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | local bad: Int = (\"wrong\")\n               ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: Int = (\"wrong\")\n                      ^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "local-bool",
    "oracleCaseId": "local-bool",
    "source": "local bad: Int = (true)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "fcbdb0e1b935db23cfcf73a9c915f62b088f46079083fac10bff1600eb97f20a",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b893ae4b6db7f2aa60b49118b7efacc4a6c5cf7ae51276a5e796444d500daa8c"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Boolean`.\nValue: true\n\n1 | local bad: Int = (true)\n               ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: Int = (true)\n                      ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "local-null",
    "oracleCaseId": "local-null",
    "source": "local bad: Int = (null)\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "c7d8bac40e814b80a91284003f754104db2f423964352a6a46b63d01b8089baa",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "de7f2d343e2ef66ed7f63d944d87c62153e586c40075946da3ef1e67aea2a036"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got `null`.\n\n1 | local bad: Int = (null)\n               ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: Int = (null)\n                      ^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "class-public-string",
    "oracleCaseId": "class-public-string",
    "source": "class C { bad: Int = (\"wrong\") }\nresult = new C {}.bad\n",
    "expression": "result",
    "sourceSha256": "7d146cf0ac77bdd21b93d6ea12aa3e8d7d0050b1f9932689a4bbc001c3d8cb05",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9fb7550a0de9069451a6fb46545cb2e404781839d35f4de651989d32a2172a70"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | class C { bad: Int = (\"wrong\") }\n                   ^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n1 | class C { bad: Int = (\"wrong\") }\n                         ^^^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | result = new C {}.bad\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "class-public-bool",
    "oracleCaseId": "class-public-bool",
    "source": "class C { bad: Int = (true) }\nresult = new C {}.bad\n",
    "expression": "result",
    "sourceSha256": "a45c85c855346c41b967cbefacea2ee4f6ec922deac1e4dac4a803400245effb",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "183ba0a67a4eab6a04c1ed99bef686988528bd11bb306ab6c439c2ef0f9781ad"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Boolean`.\nValue: true\n\n1 | class C { bad: Int = (true) }\n                   ^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n1 | class C { bad: Int = (true) }\n                         ^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | result = new C {}.bad\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "class-public-null",
    "oracleCaseId": "class-public-null",
    "source": "class C { bad: Int = (null) }\nresult = new C {}.bad\n",
    "expression": "result",
    "sourceSha256": "0bbda97d35be5b58f25a81111030ad24f7212e33a789392602d636ea3d32bdc8",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c7c3586f4817980491345b78cc631040d9fb48f5f59f80036f00a172ee119c4a"
    },
    "oracleAcquisition": "target/m2/stage31-platform-oracle-extra-02",
    "oracleReportSha256": "a0030e83f8fcf430d68b5a73e964c2c027235906fac2dfa0948b524d54bde2a0",
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got `null`.\n\n1 | class C { bad: Int = (null) }\n                   ^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n1 | class C { bad: Int = (null) }\n                         ^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n2 | result = new C {}.bad\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  }
];

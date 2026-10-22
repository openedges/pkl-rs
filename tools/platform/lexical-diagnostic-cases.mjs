// Complete pinned Java 0.32.0 setup-error captures at the identical URI.
// Each API call constructs a fresh evaluator; recovery calls share the API instance.
export const lexicalDiagnosticContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "oracle": {
    "acquisition": "target/m2/stage27-platform-lexical-java-01",
    "acquisitionReportSha256": "62cbb02ea121026605252f2b94568f68ce20c9a96dae94a759b8092ecc0d82c1",
    "oracleMetadataSha256": "18f25c1cdcdb4e47cb894adc172e7af1e526c9cf38b8212d0c151fe34f36a7a2",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d"
  }
};

export const lexicalDiagnosticCases = [
  {
    "id": "string-stage26-inner-invalid-escape",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"bad\\q\")\")\")\")\")\")\")\")\"\n",
    "expression": "result",
    "sourceSha256": "d7c321b1e45448483f4dc120a5bd04885b3fede658969894b8fbec831c3030bb",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ae07b9d291f589d2f7f877993e03796603b078e3d19abe8edd11b78a9a6707fb",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"bad\\q\")\")\")\")\")\")\")\")\"\n                                         ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-first-lexical-error",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"bad\\q\")\")\")\")\")\")\")\")\")\\(\"bad\\z\")\"\n",
    "expression": "result",
    "sourceSha256": "25c80ceee0821452adbd73e573526d8013277d71ae2bd5acae76d7e0aeca80ca",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "075345b2605420192b2f27369b8250f4abd69c02b7eced67088f058371ae217a",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"bad\\q\")\")\")\")\")\")\")\")\")\\(\"bad\\z\")\"\n                                            ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-unterminated-inner-string",
    "source": "result = \"outer\\(\"inner\n",
    "expression": "result",
    "sourceSha256": "9d1e7068b589e8e9bb91a1fed24b6cd65d091bfc5d9f3ade3dd96bc2483ee14f",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1a3c0b9e2fee634eb9660bf1260b01e6ec1b924f64878280951756b6707655a3",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nMissing `\"` delimiter.\n\n1 | result = \"outer\\(\"inner\n                           ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-unterminated-comment",
    "source": "result = \"outer\\( /* missing close )\"\n",
    "expression": "result",
    "sourceSha256": "c42e3ac70697abc8a03befa805c790f8c965843a0242de27e382f64fb7b360be",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "14d6d04a3064239ce3dffa39137ad9554fa772ee50007dc2f8c70ba1845591c4",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected end of file.\n\n2 | \n    ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-nested-syntax-before-later-escape",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(1 + \"bad\\q\")\")\")\")\")\")\")\")\"\n",
    "expression": "result",
    "sourceSha256": "1aec1522c839ac88bffb0de97f16dca5bcc7ceb0d3adc0e833a948a3d41d8611",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ce0834e2d718f8f1450eac4af2575978de0f98e79869f0d4bb0db749143ed31f",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(1 + \"bad\\q\")\")\")\")\")\")\")\")\"\n                                             ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-inner-lexical-before-missing-close",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"bad\\q\")\")\")\")\")\")\")\"\n",
    "expression": "result",
    "sourceSha256": "f628190f932933de0bd1d16e1cd208d4166b317f3a1b86f4db4e59ced308e808",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "89d178f834e4394cfc4201357dba4ae5a42db5ce551b7ab5529ece89f6a44e0a",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"bad\\q\")\")\")\")\")\")\")\"\n                                         ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-boundary-unclosed-interpolation",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(42)\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\n",
    "expression": "result",
    "sourceSha256": "b03f8220a454881fb01a0264707de4fe935844d94496d8f76a7d683a6ce2afab",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9b091cb58da602dd36b8a4e8f64363a8a72aec94bccf7edcca61810140d2e3b6",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nMissing `\"` delimiter.\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(42)\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\n                                                                                                                                                                       ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-boundary-unclosed-inner-string",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"inner\n",
    "expression": "result",
    "sourceSha256": "343096eee338ca60f374eafbfe0863ed4117c00b5146ed4f82284e69050c8a1c",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "106686c2f9befa2322be78f41a1af76e3a9f967f15dc1eb763d50d2f57e45826",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nMissing `\"` delimiter.\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"inner\n                                                                                                             ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-boundary-unclosed-comment",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(/* unterminated )\"\n",
    "expression": "result",
    "sourceSha256": "8bc78f2482607438b6c1e7db2f1d80113fb1b69b9d48c9b7d4dcfbae2cb0d650",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "14d6d04a3064239ce3dffa39137ad9554fa772ee50007dc2f8c70ba1845591c4",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected end of file.\n\n2 | \n    ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-multiline-opening-24",
    "source": "result = \"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(42)\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "f1677b3c4dfc5d531e7cc30617dbf5f789f14d2527621805bd1c21e197f74aff",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5e3a1d1edf5dd7f9b4bf3da31db02cfe562142ec8d8caefb81e4866100d642c5",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nThe content of a multi-line string must begin on a new line.\n\n1 | result = \"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(42)\n                                                                                                                                                          ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-multiline-opening-31",
    "source": "result = \"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(42)\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "7b68aa3127d15c5ea16d2c93bf51b6dc82f545cf87c736b38856b3a73c41541d",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ce3e65fcd41c978960ceadb184c2a666641fc56af615ece4446487dc80fef300",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nThe content of a multi-line string must begin on a new line.\n\n1 | result = \"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(\"\"\"x\\(42)\n                                                                                                                                                                                                    ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-multiline-closing-24",
    "source": "result = \"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(42)x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\"\n",
    "expression": "result",
    "sourceSha256": "7c29f7aa72aa9f26114dcc81fceff7ab39d33db1fef717995af65a4708458a24",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8a328e35bc566ca0c0c14e0033e58aee412fd76c61219d83843ed639232b512a",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nThe closing delimiter of a multi-line string must begin on a new line.\n\n24 | \\(\"\"\"\n       ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-multiline-closing-31",
    "source": "result = \"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(42)x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\")x\"\"\"\n",
    "expression": "result",
    "sourceSha256": "7c7d58bd077d4e59a3fb96c222c0fba08df6ac3d1bed519816a2c36382e8a830",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8de9e195c20a6519e9cf658b47e456fc6f761f21ded6a852095d747ad2989a17",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nThe closing delimiter of a multi-line string must begin on a new line.\n\n31 | \\(\"\"\"\n       ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-multiline-indent-24",
    "source": "result = \"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(42)\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\"\n",
    "expression": "result",
    "sourceSha256": "6c9d2e2405f5e16d3735199c86b394bb72dae3202380d3919aa5969af679426e",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "622d8a06153948bc0de3fd5edf214dbefe8bb1ff44bda83604c87c34120298d5",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nLine must match or exceed indentation of the String's last line.\n\n25 | x\\(42)\n     ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage26-multiline-indent-31",
    "source": "result = \"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(\"\"\"\nx\\(42)\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\")\n  \"\"\"\n",
    "expression": "result",
    "sourceSha256": "3437ddc22d29895bad3658a93147b7a17cc8b6f808ed6cb5aa46b6650d60afce",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "fc6d3d6f36f23832aef5d35f2417c4ae2c4936bb83fe54daa92a93b6449f68eb",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nLine must match or exceed indentation of the String's last line.\n\n32 | x\\(42)\n     ^\nat m2-platform (file:///m2-platform.pkl)\n"
  }
];

export const lexicalDiagnosticRecoveries = [
  {
    "id": "string-stage27-recovery-ordinary",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(42)\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\"\n",
    "expression": "result",
    "sourceSha256": "4b5cc14292b7f388369f211be981e6e9211012627182ba09144749cb3d31c432",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "42"
  },
  {
    "id": "string-stage27-recovery-raw",
    "source": "result = #\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(42)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#\n",
    "expression": "result",
    "sourceSha256": "7a5f066f7f41d28e7d53edeb43b36ce2a31734c3e2bb821900553f2436bcdb1b",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "42"
  },
  {
    "id": "string-stage27-recovery-multiline",
    "source": "result = \"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(42)\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "a0983e946c352e2f15bc5fba4577be03a30971015a8048b0ff67e9c1920b51f3",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "42"
  }
];

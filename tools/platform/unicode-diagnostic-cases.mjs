// Complete fresh pinned Java 0.32.0 captures at the identical platform URI.
// Each call constructs a fresh evaluator; successful work follows setup errors.
export const unicodeDiagnosticContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "oracle": {
    "acquisition": "target/m2/stage28-platform-unicode-java-01",
    "acquisitionReportSha256": "5896ca322f17b0cfdb6149d97959d053678d447464cbebf21940c44b284bedb7",
    "oracleMetadataSha256": "18f25c1cdcdb4e47cb894adc172e7af1e526c9cf38b8212d0c151fe34f36a7a2",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d"
  }
};

export const unicodeDiagnosticCases = [
  {
    "id": "string-stage28-ordinary-empty",
    "source": "result = \"\\u{}\"\n",
    "expression": "result",
    "sourceSha256": "690fd7b5fba591062168dd1abae25195d83658264207befd70ff93b920b4d51b",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "73f1dfba20f0a93875e48e3da2f1b99d969050c5222e98fa27c60a0cfe5e52f6",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = \"\\u{}\"\n              ^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-ordinary-nonhex",
    "source": "result = \"\\u{xyz}\"\n",
    "expression": "result",
    "sourceSha256": "e2840504e378078fd50456981080aba95bd8071a60ed47b6635d48faaea1b771",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "35e30d486594f9eba254e3a700a5468f85fd2b1f7d348bb778c235f501f03977",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{xyz}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = \"\\u{xyz}\"\n              ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-ordinary-missing-brace",
    "source": "result = \"\\u1234\"\n",
    "expression": "result",
    "sourceSha256": "039954fa81029820e897bf5daa7f3a882fd9e54d8e332e7093ce293b67f7f7f9",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "decac6c9ba7b78048be65513e1fda40c67ab2f911f8903791007a941a53fb787",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected character `1`. Did you mean `{`?\n\n1 | result = \"\\u1234\"\n                ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-closed-before-later-escape",
    "source": "result = \"\\u{}\\q\"\n",
    "expression": "result",
    "sourceSha256": "ffc280e9b8ad996acc06ea90ae14d1399f8c5e2cd5991edcbbf177e4311aba63",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "085d33171cb8a96314c086282989f5d0aae4860a5d72359c956b7ac513b5b80d",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = \"\\u{}\\q\"\n              ^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-syntax-before-closed",
    "source": "result = \"\\(1 + )\\u{}\"\n",
    "expression": "result",
    "sourceSha256": "25014ca33f760391e6404628a2730f3813457c2416f885b93f0dddcb53986d94",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5fce3588a0d268f309b9fcb332a24197075a38b35c3fda5fe39c5f55ee2a126d",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected token `)`. Expected `)`.\n\n1 | result = \"\\(1 + )\\u{}\"\n                    ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-multiline-closed-before-later-escape",
    "source": "result = \"\"\"\n\\u{}\\q\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "5815d75451e4b30e57db48294fabb596b1c44ead7db472ff94e37169311da0fe",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1cb16c1ca26eb1810560ddf0dde177c102b44bece7ee4f3d5c8bdcd25bd6727f",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid character escape sequence `\\q`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>\n\n2 | \\u{}\\q\n        ^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-multiline-closed-before-later-syntax",
    "source": "result = \"\"\"\n\\u{}\\(1 + )\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "6efef22e6d3c4c1867715742a12e63bddc661e1ce91fdc241451414ef2d93039",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "120be58f17196d6140a1f3afdf3cebef22fabbc15ce299004e2cc48d2ebbe878",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected token `)`. Expected `)`.\n\n2 | \\u{}\\(1 + )\n              ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-multiline-indent-before-closed",
    "source": "result = \"\"\"\n\\u{}\n  \"\"\"\n",
    "expression": "result",
    "sourceSha256": "9eb0ef4c4e4370611bcb8442b62732c95ac57583419879bfaf0f49787ff9be08",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6fa0ba1b250f406baec670947012ca843e7bb06f9be8a8f83b100fd8832730ee",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nLine must match or exceed indentation of the String's last line.\n\n2 | \\u{}\n    ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-eof-u",
    "source": "result = \"\\u",
    "expression": "result",
    "sourceSha256": "7421d907bb5e83565d5761639558c5d124dadc8039f4171f52e35dc0e8a9eb4b",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "556d15e28d8f4c900b5849915bc2495754671f86f03f5d193b06200e895c6253",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected character `EOF`. Did you mean `{`?\n\n1 | result = \"\\u\n               ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-emoji-span",
    "source": "result = \"😀\\u{xyz}\"\n",
    "expression": "result",
    "sourceSha256": "5f2c16dac59fc20ad113ffa07199f13847049e64bff3e02e1dfa84ec5d5243a5",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "569549a3bc36dfafc05f2555265b0c8ce65cdea3211609f0958251ce1b2de081",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{xyz}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = \"😀\\u{xyz}\"\n                ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-raw-two-nonhex",
    "source": "result = ##\"\\##u{xyz}\"##\n",
    "expression": "result",
    "sourceSha256": "544e207f2f701628714b4650d31e0956f3445f32f433d045f3f14574be0b2384",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "52ef4061cef861a24a96455c147e9f9ad54db9cdebacae04532db4942093bbcb",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\##u{xyz}`.\n\nValid Unicode escape sequences are \\##u{0} to \\##u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = ##\"\\##u{xyz}\"##\n                ^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-nested-emoji-span",
    "source": "result = \"😀\\(\"漢\\u{}\")\"\n",
    "expression": "result",
    "sourceSha256": "50f6faeb51d052f293a7a804f7c2cf368533180869218f4095dff7f5f6d187ac",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5f9bb8467927a0658e5ed51deb6c197ac35adfb3241b9333738cd663882d593d",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = \"😀\\(\"漢\\u{}\")\"\n                    ^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-platform-deep-ordinary-31",
    "source": "result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\u{}\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\"\n",
    "expression": "result",
    "sourceSha256": "f8a43082008f5ffa8988979443ac3a9fe777571a347b5cefca7b79e8daef7b69",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f73dc8336508b753dbb3838bee2327f851dbd4ea6cbf3790078ccd3ac285ccd8",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = \"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\(\"\\u{}\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\")\"\n                                                                                                           ^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-platform-deep-raw-31",
    "source": "result = #\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(##\"\\##u{xyz}\"##)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#\n",
    "expression": "result",
    "sourceSha256": "b450a9446bdb02549de60cf1292693fed9419eb6aafbe28946a5eb28e0eef0a0",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "aefb8e9dac2e927234afa8d7aef66d76f17091c5a0c5d33bb9865b9a709efc4a",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\##u{xyz}`.\n\nValid Unicode escape sequences are \\##u{0} to \\##u{10FFFF} (1-6 hexadecimal characters).\n\n1 | result = #\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(#\"\\#(##\"\\##u{xyz}\"##)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#)\"#\n                                                                                                                                                                           ^^^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-platform-deep-multiline-24",
    "source": "result = \"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\"\"\n\\(\"\\u{}\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\")\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "47ad1bcbf5a955c27a9d2053bd39d0fad730aff5740d4466a115782b8e5feca7",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9f06c8820daf8e86cbce7bf1741be3209dce2b01f518762af0b2eda075e8d2eb",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nInvalid Unicode escape sequence `\\u{}`.\n\nValid Unicode escape sequences are \\u{0} to \\u{10FFFF} (1-6 hexadecimal characters).\n\n25 | \\(\"\\u{}\")\n        ^^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-platform-newline-after-closed",
    "source": "result = \"\\u{}\n",
    "expression": "result",
    "sourceSha256": "5148b572d774a39eb07b68f8e83af3170cd26603c867c6ad18b0431e1e4136ee",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b2b222c24ec21861c385943029ffc10ec89ecead1f01dbede19fe14d7f7512f9",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nMissing `\"` delimiter.\n\n1 | result = \"\\u{}\n                  ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-platform-text-newline-after-closed",
    "source": "result = \"\\u{}x\n",
    "expression": "result",
    "sourceSha256": "4dad8812be8f9d141d267b10654bbbe1dd889ff6373457b1cc7db834ede35f87",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3589a320235e27102cd0704a0e11058de5791a1df7e9b5166d46027a6a502e6e",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nMissing `\"` delimiter.\n\n1 | result = \"\\u{}x\n                   ^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "string-stage28-platform-multiline-after-close-comment-eof",
    "source": "result = \"\"\"\n\\u{}\n\"\"\" /*",
    "expression": "result",
    "sourceSha256": "3cbc488e13b27b9db8dc94824627379589af46093642f215f1e03175e5b84774",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "58f94aff6c0b84aceab1bb157cb5b2b55c396bfb9a2b4f010f8bb21f21714ced",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nUnexpected end of file.\n\n3 | \"\"\" /*\n          ^\nat m2-platform (file:///m2-platform.pkl)\n"
  }
];

export const unicodeDiagnosticRecoveries = [
  {
    "id": "string-stage28-recovery-ordinary",
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
    "id": "string-stage28-recovery-raw",
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
    "id": "string-stage28-recovery-multiline",
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


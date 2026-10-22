// Complete fresh pinned Java 0.32.0 captures at the identical platform URI.
// Literal UTF-16 parent identities keep nested entry and lambda counters separate.
export const diagnosticFrameIdentityContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "oracle": {
    "acquisition": "target/m2/stage30-platform-oracle-01",
    "acquisitionReportSha256": "1e910742172effe9f42fe64bdf3a804b9dd790366591157f884e355a8d9e516e",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d"
  }
};

export const diagnosticFrameIdentityCases = [
  {
    "id": "diagnostic-frame-stage30-listing-question",
    "source": "x = new Mapping { [\"\\u{d800}\"] = new Listing { throw(\"first\") }; [\"?\"] = new Listing { throw(\"second\") } }\nresult = x[\"?\"][0]\n",
    "expression": "result",
    "sourceSha256": "f4585095b48b422397db8e07b30677eefb492f226a19e434e9c168fb687bd0f6",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "00cad28007045e9789341f79690ae59d978c454e2bdabb7ffa789f12949312de"
    },
    "expectedError": "–– Pkl Error ––\nsecond\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new Listing { throw(\"first\") }; [\"?\"] = new Listing { throw(\"second\") } }\n                                                                                           ^^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"][#1] (file:///m2-platform.pkl)\n\n2 | result = x[\"?\"][0]\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-listing-low",
    "source": "x = new Mapping { [\"\\u{d800}\"] = new Listing { throw(\"first\") }; [\"?\"] = new Listing { throw(\"second\") }; [\"\\u{dc00}\"] = new Listing { throw(\"third\") } }\nresult = x[\"\\u{dc00}\"][0]\n",
    "expression": "result",
    "sourceSha256": "9b5f07118c5772e2d8bec86ef11a04fb422e27ed5948bd111dbf50f9a0d4a7ce",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4cc46e2f2bbf6f5fb3b02d14d12bf55f863a0bbd1272cd319bfa75bbd0bd8ada"
    },
    "expectedError": "–– Pkl Error ––\nthird\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new Listing { throw(\"first\") }; [\"?\"] = new Listing { throw(\"second\") }; [\"\\u{dc00}\"] = new Listing { throw(\"third\") } }\n                                                                                                                                           ^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"][#1] (file:///m2-platform.pkl)\n\n2 | result = x[\"\\u{dc00}\"][0]\n             ^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-dynamic-question",
    "source": "local k = \"k\"\nx = new Mapping { [\"\\u{d800}\"] = new Mapping { [k] = throw(\"first\") }; [\"?\"] = new Mapping { [k] = throw(\"second\") } }\nresult = x[\"?\"][\"k\"]\n",
    "expression": "result",
    "sourceSha256": "17b630a464e6bb042124cf336efe13bfb068959571ec69cd246e6768b7f42190",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a999bf995206c2e420270212cddd9056e221d9169b58c27a0046bc18752aa04b"
    },
    "expectedError": "–– Pkl Error ––\nsecond\n\n2 | x = new Mapping { [\"\\u{d800}\"] = new Mapping { [k] = throw(\"first\") }; [\"?\"] = new Mapping { [k] = throw(\"second\") } }\n                                                                                                       ^^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"][#1] (file:///m2-platform.pkl)\n\n3 | result = x[\"?\"][\"k\"]\n             ^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-dynamic-high",
    "source": "local k = \"k\"\nx = new Mapping { [\"\\u{d800}\"] = new Mapping { [k] = throw(\"first\") }; [\"?\"] = new Mapping { [k] = throw(\"second\") } }\nresult = x[\"\\u{d800}\"][\"k\"]\n",
    "expression": "result",
    "sourceSha256": "b4a5816d56c704f326661ba5bb351045fb4928124ac21d2a5a517cca4d2b482c",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "545739a30f3e22568c6782f9d1c49401ff00c53a245b298115f56fd57af1649a"
    },
    "expectedError": "–– Pkl Error ––\nfirst\n\n2 | x = new Mapping { [\"\\u{d800}\"] = new Mapping { [k] = throw(\"first\") }; [\"?\"] = new Mapping { [k] = throw(\"second\") } }\n                                                         ^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"][#1] (file:///m2-platform.pkl)\n\n3 | result = x[\"\\u{d800}\"][\"k\"]\n             ^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-lambda-question",
    "source": "x = new Mapping { [\"\\u{d800}\"] = (a) -> throw(\"first\"); [\"?\"] = (b) -> throw(\"second\") }\nresult = x[\"?\"].apply(0)\n",
    "expression": "result",
    "sourceSha256": "61917ec07f9eaf1d8e95832789e9a9e32e801f02fbb8af98d3620eef25d8c1c7",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b19ef968dcfe03d001716e051f1332dc2c9125d768dbd1155faf55483eb8f0ff"
    },
    "expectedError": "–– Pkl Error ––\nsecond\n\n1 | x = new Mapping { [\"\\u{d800}\"] = (a) -> throw(\"first\"); [\"?\"] = (b) -> throw(\"second\") }\n                                                                           ^^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"].<function#1> (file:///m2-platform.pkl)\n\n2 | result = x[\"?\"].apply(0)\n             ^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-lambda-low",
    "source": "x = new Mapping { [\"\\u{d800}\"] = (a) -> throw(\"first\"); [\"?\"] = (b) -> throw(\"second\"); [\"\\u{dc00}\"] = (c) -> throw(\"third\") }\nresult = x[\"\\u{dc00}\"].apply(0)\n",
    "expression": "result",
    "sourceSha256": "0fdb7c10669c59e5d2cb087c477da46159336db752560e39455b21fb065401b7",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e55c37ea257bdb49e78e2cc20817aaadf8888e6b8d05cfb1eb9d46ce3f4f529d"
    },
    "expectedError": "–– Pkl Error ––\nthird\n\n1 | x = new Mapping { [\"\\u{d800}\"] = (a) -> throw(\"first\"); [\"?\"] = (b) -> throw(\"second\"); [\"\\u{dc00}\"] = (c) -> throw(\"third\") }\n                                                                                                                  ^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"].<function#1> (file:///m2-platform.pkl)\n\n2 | result = x[\"\\u{dc00}\"].apply(0)\n             ^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-entry-nested",
    "source": "value = new Mapping { [\"\\u{d800}\"] = new { bad = throw(\"stop\") } }\nresult = value[\"\\u{d800}\"].bad\n",
    "expression": "result",
    "sourceSha256": "4fde9a46f38c69984c888687e5fe4a568587ddd9e334285fa3d3ce4865a6ec99",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "abb996c2471f79a9b67ae7e28d2ffb560a64c3d454b6cca2692cb74cdcc5b98b"
    },
    "expectedError": "–– Pkl Error ––\nstop\n\n1 | value = new Mapping { [\"\\u{d800}\"] = new { bad = throw(\"stop\") } }\n                                                     ^^^^^^^^^^^^^\nat m2-platform#value[\"?\"].bad (file:///m2-platform.pkl)\n\n2 | result = value[\"\\u{d800}\"].bad\n             ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-entry-let",
    "source": "value = new Mapping { [\"\\u{d800}\"] = let (x = 1) throw(\"stop\") }\nresult = value[\"\\u{d800}\"]\n",
    "expression": "result",
    "sourceSha256": "69695e98237562462d4248f36f6c885d7808b355957a3d2a310991473902b3b5",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "09f5b8f994635675c9c42362ddc4c877417e3b107efb5ed5b82e60e4b48c10c0"
    },
    "expectedError": "–– Pkl Error ––\nstop\n\n1 | value = new Mapping { [\"\\u{d800}\"] = let (x = 1) throw(\"stop\") }\n                                                     ^^^^^^^^^^^^^\nat m2-platform#value[\"?\"] (file:///m2-platform.pkl)\n\n1 | value = new Mapping { [\"\\u{d800}\"] = let (x = 1) throw(\"stop\") }\n                                         ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#value[\"?\"].<let expr> (file:///m2-platform.pkl)\n\n2 | result = value[\"\\u{d800}\"]\n             ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-entry-reversed",
    "source": "value = new Mapping { [\"\\u{dc00}\\u{d800}\"] = throw(\"stop\") }\nresult = value[\"\\u{dc00}\\u{d800}\"]\n",
    "expression": "result",
    "sourceSha256": "58e5e5ab9d1e253e7e9d37f764b937b3d56ef0060c6807f0af308d71ace6ca40",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "344efb1d585d0b7bd6a9d331ebfa438c04094f5e87c76db2bfe600e48e892fef"
    },
    "expectedError": "–– Pkl Error ––\nstop\n\n1 | value = new Mapping { [\"\\u{dc00}\\u{d800}\"] = throw(\"stop\") }\n                                                 ^^^^^^^^^^^^^\nat m2-platform#value[\"??\"] (file:///m2-platform.pkl)\n\n2 | result = value[\"\\u{dc00}\\u{d800}\"]\n             ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-entry-pair",
    "source": "value = new Mapping { [\"\\u{d800}\\u{dc00}\"] = throw(\"stop\") }\nresult = value[\"\\u{10000}\"]\n",
    "expression": "result",
    "sourceSha256": "56f6d4dcfff0117da8c90ceb75102dde2617379933e099beb84b32b878c8f5f7",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ad5cd8d90fa6a499486a22069d8f247404e11ad45986128f8a370b494f5f113b"
    },
    "expectedError": "–– Pkl Error ––\nstop\n\n1 | value = new Mapping { [\"\\u{d800}\\u{dc00}\"] = throw(\"stop\") }\n                                                 ^^^^^^^^^^^^^\nat m2-platform#value[\"𐀀\"] (file:///m2-platform.pkl)\n\n2 | result = value[\"\\u{10000}\"]\n             ^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-listing-replacement-pair-control",
    "source": "x = new Mapping { [\"\\u{fffd}\"] = new Listing { throw(\"replacement\") }; [\"\\u{d800}\\u{dc00}\"] = new Listing { throw(\"pair\") }; [\"?\"] = new Listing { throw(\"question\") } }\nresult = x[\"?\"][0]\n",
    "expression": "result",
    "sourceSha256": "cab1755fc9872293ecc37fde876dc39cca46b18254fbceb50410763d667d36e2",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9a3eb2e887de0cd251ee56c910cf2eaa569a41635567289871456756b9b54c52"
    },
    "expectedError": "–– Pkl Error ––\nquestion\n\n1 | x = new Mapping { [\"\\u{fffd}\"] = new Listing { throw(\"replacement\") }; [\"\\u{d800}\\u{dc00}\"] = new Listing { throw(\"pair\") }; [\"?\"] = new Listing { throw(\"question\") } }\n                                                                                                                                                       ^^^^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"][#1] (file:///m2-platform.pkl)\n\n2 | result = x[\"?\"][0]\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-overflow-high",
    "source": "x = new Mapping { [\"\\u{d800}\"] = 9223372036854775808 }\nresult = x[\"\\u{d800}\"]\n",
    "expression": "result",
    "sourceSha256": "80892186cbaa78330a7668dd7c3d041ff0d0b7a734b11975461b259c0f04fc9b",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-extra-01",
    "oracleAcquisitionReportSha256": "7ccb3ace05f831eb3a4716d3d1e5ba5ea053f129cce8bd54cb3e0c7441bdfe5a",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c2b589d4ccd3dfe1e0cf400461bf21aeb25c82a0c7ff68704c13aebeca7a82ca"
    },
    "expectedError": "–– Pkl Error ––\nInt literal `9223372036854775808` is too large.\n\n1 | x = new Mapping { [\"\\u{d800}\"] = 9223372036854775808 }\n                                     ^^^^^^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"] (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-frame-stage30-listing-high-control",
    "source": "x = new Mapping { [\"\\u{d800}\"] = new Listing { throw(\"first\") }; [\"?\"] = new Listing { throw(\"second\") } }\nresult = x[\"\\u{d800}\"][0]\n",
    "expression": "result",
    "sourceSha256": "a9e988d78d74e5c675eec297061c84237a938ad9dc728ee44a871767e6b1d3a2",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-extra-01",
    "oracleAcquisitionReportSha256": "7ccb3ace05f831eb3a4716d3d1e5ba5ea053f129cce8bd54cb3e0c7441bdfe5a",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a1da16337e385b57779d2aed08ee0a77fa0d7844696858f1f8d94d86199a455d"
    },
    "expectedError": "–– Pkl Error ––\nfirst\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new Listing { throw(\"first\") }; [\"?\"] = new Listing { throw(\"second\") } }\n                                                   ^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"][#1] (file:///m2-platform.pkl)\n\n2 | result = x[\"\\u{d800}\"][0]\n             ^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-local-method-high",
    "source": "x = new Mapping { [\"\\u{d800}\"] = new { local function fail() = throw(\"method\"); bad = fail() } }\nresult = x[\"\\u{d800}\"].bad\n",
    "expression": "result",
    "sourceSha256": "791b3958689ed20aaeefcc62cfb7bb1c8abe0b164a7a3e092b426ed3c74f55da",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-extra-02",
    "oracleAcquisitionReportSha256": "73b5e69f43700112475c875b0e9b54cd6bc349598551d009846b849c78cf9650",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "741cc19b7d40c469fcc9197917ea0302bb248753dc6bbea9a68ee2d3ace87fc7"
    },
    "expectedError": "–– Pkl Error ––\nmethod\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new { local function fail() = throw(\"method\"); bad = fail() } }\n                                                                   ^^^^^^^^^^^^^^^\nat m2-platform#x[\"?\"].fail (file:///m2-platform.pkl)\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new { local function fail() = throw(\"method\"); bad = fail() } }\n                                                                                          ^^^^^^\nat m2-platform#x[\"?\"].bad (file:///m2-platform.pkl)\n\n2 | result = x[\"\\u{d800}\"].bad\n             ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-local-typed-high",
    "source": "x = new Mapping { [\"\\u{d800}\"] = new { local bad: Int = \"wrong\"; checked = bad } }\nresult = x[\"\\u{d800}\"].checked\n",
    "expression": "result",
    "sourceSha256": "645b83cc8301b44553adfaf50ef59417e60c09c918b1d3192d22e8b99b669716",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-extra-03",
    "oracleAcquisitionReportSha256": "08f7d53205042ee664e33aeb50c185f32d29fe6dad81ec5f024dfdbd8863c2d1",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c84d7c5bee26b7e64a946c8a049089896b9e9fe0de5778fa49a3ad7b103af0b3"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new { local bad: Int = \"wrong\"; checked = bad } }\n                                                      ^^^\nat m2-platform#x[\"?\"].bad (file:///m2-platform.pkl)\n\n1 | x = new Mapping { [\"\\u{d800}\"] = new { local bad: Int = \"wrong\"; checked = bad } }\n                                                                               ^^^\nat m2-platform#x[\"?\"].checked (file:///m2-platform.pkl)\n\n2 | result = x[\"\\u{d800}\"].checked\n             ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-local-typed-ascii",
    "source": "x = new { local bad: Int = \"wrong\"; checked = bad }\nresult = x.checked\n",
    "expression": "result",
    "sourceSha256": "716170baea814fa460f2a8e99bef68a5eb6f3feb010fec9d2fa9cf4d0e0021d9",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-controls-01",
    "oracleAcquisitionReportSha256": "30bd818b13712f1b0b84305475c152f9997c945a183b100b272377c0805d600d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "86e9bc11b9d1a43d2b32eed68d092e1db3944a576f6679c19ae5d77503440224"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | x = new { local bad: Int = \"wrong\"; checked = bad }\n                         ^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n\n1 | x = new { local bad: Int = \"wrong\"; checked = bad }\n                                                  ^^^\nat m2-platform#x.checked (file:///m2-platform.pkl)\n\n2 | result = x.checked\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-class-typed-ascii",
    "source": "class C { bad: Int = \"wrong\" }\nx = new C {}\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "a69e1dcb8a6cc67ea8d296234e3c4b99cc34dc74ff484aeeb57bd8d572ca7c30",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-controls-01",
    "oracleAcquisitionReportSha256": "30bd818b13712f1b0b84305475c152f9997c945a183b100b272377c0805d600d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "2a9c0e8f424f5f85643182be2de355ee0d0a6505badcd5e365abb0eccb48cc3f"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | class C { bad: Int = \"wrong\" }\n                   ^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n1 | class C { bad: Int = \"wrong\" }\n                         ^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n3 | result = x.bad\n             ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-overflow-dynamic-body",
    "source": "local key = \"computed\"\nx = new Mapping { [key] = 9223372036854775808 }\nresult = x[\"computed\"]\n",
    "expression": "result",
    "sourceSha256": "aa8399a70ae0d330da7b5e52d014daa558eb12e17606d54d7959fecd8e7b1a6b",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-controls-01",
    "oracleAcquisitionReportSha256": "30bd818b13712f1b0b84305475c152f9997c945a183b100b272377c0805d600d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c45d793b0d4d6b58894bcb7b8310e6b181391546a297e09c22ebd45eb4fff9d8"
    },
    "expectedError": "–– Pkl Error ––\nInt literal `9223372036854775808` is too large.\n\n2 | x = new Mapping { [key] = 9223372036854775808 }\n                              ^^^^^^^^^^^^^^^^^^^\nat m2-platform#x[#1] (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-frame-stage30-overflow-element-body",
    "source": "x = new Listing { 9223372036854775808 }\nresult = x[0]\n",
    "expression": "result",
    "sourceSha256": "cc941439d236ed428c9b3a1f4015ef82b5f5eb6166327f065891856838ad93e5",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-controls-01",
    "oracleAcquisitionReportSha256": "30bd818b13712f1b0b84305475c152f9997c945a183b100b272377c0805d600d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "17e9b31e435db5879e614200973f8d9231c8bc117e0556b161efbf052b7f5e67"
    },
    "expectedError": "–– Pkl Error ––\nInt literal `9223372036854775808` is too large.\n\n1 | x = new Listing { 9223372036854775808 }\n                      ^^^^^^^^^^^^^^^^^^^\nat m2-platform#x[#1] (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-frame-stage30-overflow-key-expression",
    "source": "x = new Mapping { [9223372036854775808] = 1 }\nresult = x[0]\n",
    "expression": "result",
    "sourceSha256": "d2fc0a83c5d8d8073474dba010105bd3d82d1102603b9468cfd98643462d45f1",
    "oracleAcquisition": "target/m2/stage30-platform-oracle-controls-01",
    "oracleAcquisitionReportSha256": "30bd818b13712f1b0b84305475c152f9997c945a183b100b272377c0805d600d",
    "captureSha256": {
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "57ca1514e80612bad4412361f4e2664fe915448dcbd58140c0a434b96608256c"
    },
    "expectedError": "–– Pkl Error ––\nInt literal `9223372036854775808` is too large.\n\n1 | x = new Mapping { [9223372036854775808] = 1 }\n                       ^^^^^^^^^^^^^^^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-frame-stage30-module-local-typed-ascii",
    "source": "local bad: Int = \"wrong\"\nresult = bad\n",
    "expression": "result",
    "sourceSha256": "c9c1b4ec66cdeb2662ed70bc9694c17982627ea3f17545fae6c11b88aa6471ea",
    "oracleAcquisition": "target/m2/stage30-alias-frame-regression-oracle-01",
    "oracleAcquisitionReportSha256": "ecc6ec05aff8eff83f2a874be1417385e437beb7ef10889908a1e601d22d87ed",
    "oracleCaseId": "direct-local-int",
    "oracleCallIndex": 0,
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b9352c411ca304fc0486dbeb3157daf95fec9fbf6fad14d8e7dad71dcf2f6e47"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | local bad: Int = \"wrong\"\n               ^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | local bad: Int = \"wrong\"\n                     ^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n2 | result = bad\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-class-local-typed-ascii",
    "source": "class C { local bad: Int = \"wrong\"; checked = bad }\nx = new C {}\nresult = x.checked\n",
    "expression": "result",
    "sourceSha256": "30b74efbeb9020dfbc2341f6087c0d4edf86d27494f66426d20b7a4f4bb07efb",
    "oracleAcquisition": "target/m2/stage30-alias-frame-regression-oracle-03",
    "oracleAcquisitionReportSha256": "af149f078b7abae782b0621b0d398d11d536057b21d65ddbddaa68cfe1540e10",
    "oracleCaseId": "class-local-int",
    "oracleCallIndex": 0,
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "41a4918d1a5f1cdfd2fd140318cbc53a8f90d850768ea24ff045c8f4ddda54e8"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"wrong\"\n\n1 | class C { local bad: Int = \"wrong\"; checked = bad }\n                         ^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n1 | class C { local bad: Int = \"wrong\"; checked = bad }\n                               ^^^^^^^\nat m2-platform#C.bad (file:///m2-platform.pkl)\n\n1 | class C { local bad: Int = \"wrong\"; checked = bad }\n                                                  ^^^\nat m2-platform#C.checked (file:///m2-platform.pkl)\n\n3 | result = x.checked\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "diagnostic-frame-stage30-object-local-typed-alias-function",
    "source": "typealias Fn = (Int) -> String\nx = new { local bad: Fn = () -> \"bad\"; checked = bad }\nresult = x.checked\n",
    "expression": "result",
    "sourceSha256": "4ce7282d9eafb50ef7c83b82b6b44020b2f5484730fff1c85d39ef4e4e238393",
    "oracleAcquisition": "target/m2/stage30-alias-frame-regression-oracle-02",
    "oracleAcquisitionReportSha256": "f5e50a70574052062bbfa196ebcb32f1079196cf9f9b10293c49cbc015ecdbe0",
    "oracleCaseId": "object-local-alias-function",
    "oracleCallIndex": 0,
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "64453cb081303634e1fe2ab2dc5c2f8a557ec36e9c50d01ce71472959cc768c2"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `Function1`, but got type `Function0`.\nValue: new Function0 {}\n\n1 | typealias Fn = (Int) -> String\n                   ^^^^^^^^^^^^^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n\n2 | x = new { local bad: Fn = () -> \"bad\"; checked = bad }\n                                                     ^^^\nat m2-platform#x.checked (file:///m2-platform.pkl)\n\n3 | result = x.checked\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  }
];

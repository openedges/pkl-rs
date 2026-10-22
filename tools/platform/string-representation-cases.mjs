// Complete fresh pinned Java 0.32.0 UTF-8 captures at the identical platform URI.
// Each observation uses a fresh evaluator; UTF-16 identity is checked by expressions.
export const stringRepresentationContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "oracle": {
    "acquisition": "target/m2/stage29-platform-string-java-01",
    "acquisitionReportSha256": "3d1cd5ebff546e39da70e863b66d9cffb82c7bfb6acff8aaaac3025e0bdca343",
    "oracleMetadataSha256": "18f25c1cdcdb4e47cb894adc172e7af1e526c9cf38b8212d0c151fe34f36a7a2",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d"
  }
};

export const stringRepresentationCases = [
  {
    "id": "string-stage29-high",
    "source": "result = \"\\u{D800}\"\n",
    "expression": "result",
    "sourceSha256": "19e1d1a0dfdbc52ab7e05d10f0cd2357ad4955e662500e6b565beaea9de610f7",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "8a8de823d5ed3e12746a62ef169bcf372be0ca44f0a1236abc35df05d96928e1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "?"
  },
  {
    "id": "string-stage29-low",
    "source": "result = \"\\u{DFFF}\"\n",
    "expression": "result",
    "sourceSha256": "c920e9b920d079591fa08047a5fa8c905a3b0c2babcac0c936360749cedec821",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "8a8de823d5ed3e12746a62ef169bcf372be0ca44f0a1236abc35df05d96928e1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "?"
  },
  {
    "id": "string-stage29-pair",
    "source": "result = \"\\u{D83D}\\u{DE00}\"\n",
    "expression": "result",
    "sourceSha256": "4b061877d5ed9fbc29d85dc4b5125729a47a20a7b2f7930775c8f6c495f6c9d2",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "f0443a342c5ef54783a111b51ba56c938e474c32324d90c3a60c9c8e3a37e2d9",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "😀"
  },
  {
    "id": "string-stage29-reverse",
    "source": "result = \"\\u{DE00}\\u{D83D}\"\n",
    "expression": "result",
    "sourceSha256": "a64c3cb3d2792f1a7803f3ecbe1e48e25afbded5267fb17240776c02dbe91fa0",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "e270aeb347f2165574c3a5c5bf11d038bcd3acd5abfdb5ae8a1b52d91cb842f0",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "??"
  },
  {
    "id": "string-stage29-lengths",
    "source": "result = List(\"\\u{D800}\".length,\"\\u{DFFF}\".length,\"\\u{D83D}\\u{DE00}\".length,\"\\u{DE00}\\u{D83D}\".length)\n",
    "expression": "result",
    "sourceSha256": "556f5dcf2c342dc0406f8bc0b1d08028894976eb7bc0cc1ef394c8305bc05a4d",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "e0cc302925aa99c3092b2820415232dd7ee3996ae69a3477a601ee3103618592",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(1, 1, 1, 2)"
  },
  {
    "id": "string-stage29-equality-controls",
    "source": "result = List(\"\\u{D800}\"==\"?\",\"\\u{D800}\"==\"\\u{FFFD}\",\"\\u{D800}\"==\"\\u{DFFF}\",\"\\u{D800}\"==\"\\u{D800}\")\n",
    "expression": "result",
    "sourceSha256": "f17926c6e543dc00c223f6e3ab788db8b93ca4054c91c4d912c35d172905ea74",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "f9eeff0ff8010f6eeba45e82b282692575536d6a3b369889d820a8fe30a28842",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(false, false, false, true)"
  },
  {
    "id": "string-stage29-concat-pair-equality",
    "source": "result = (\"\\u{D83D}\"+\"\\u{DE00}\")==\"\\u{1F600}\"\n",
    "expression": "result",
    "sourceSha256": "f37c5a0e155c580676cfd032b59de5ff67fbaca01392f0c80c0f658c8a243f4f",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "true"
  },
  {
    "id": "string-stage29-interpolation-pair",
    "source": "result = \"\\(\"\\u{D83D}\")\\(\"\\u{DE00}\")\"\n",
    "expression": "result",
    "sourceSha256": "0f3245c81e76436c8b9d5eff3a3c80b31d5caf2c1d3984e1f6e53897b1080f04",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "f0443a342c5ef54783a111b51ba56c938e474c32324d90c3a60c9c8e3a37e2d9",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "😀"
  },
  {
    "id": "string-stage29-ordering",
    "source": "result = List(\"\\u{D800}\"<\"\\u{DFFF}\",\"\\u{D800}\"<\"\\u{FFFD}\",\"\\u{1F600}\"<\"\\u{E000}\")\n",
    "expression": "result",
    "sourceSha256": "abe1009bd24c695bd6d213e7ffe5b737d2f40833aa446ed0eb776e7e1b6cad6c",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "898a867e248868a353c9bcddb9e510a7bebcbfecfad475a1ad15c546ff62979f",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(true, true, true)"
  },
  {
    "id": "string-stage29-list-render",
    "source": "result = List(\"\\u{D800}\",\"\\u{DFFF}\",\"\\u{D83D}\\u{DE00}\",\"\\u{DE00}\\u{D83D}\",\"?\",\"\\u{FFFD}\")\n",
    "expression": "result",
    "sourceSha256": "a1c70828bb6619aaa7bcf5d91168ecf9107bb746cbe35e5cc87aa2b4e12a3bf5",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "58a77933e5bf4354d7e1602cb5d334185f8f5235941436097f8ab20cea818822",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(\"?\", \"?\", \"😀\", \"??\", \"?\", \"�\")"
  },
  {
    "id": "string-stage29-set-identity",
    "source": "result = Set(\"\\u{D800}\",\"\\u{DFFF}\",\"?\",\"\\u{FFFD}\").length\n",
    "expression": "result",
    "sourceSha256": "38ff1e5bb035af76d5b6de810989c9aa087f8bc9edabed66adc163aa0a8b5bb1",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "4"
  },
  {
    "id": "string-stage29-set-pair-coalescing",
    "source": "result = Set(\"\\u{D83D}\\u{DE00}\",\"\\u{1F600}\",\"\\u{D83D}\"+\"\\u{DE00}\").length\n",
    "expression": "result",
    "sourceSha256": "7224ea889af5879d43f8b938a3fad9a99a765dea98b07df5d7c02fe53a338202",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "1"
  },
  {
    "id": "string-stage29-map-lookup",
    "source": "result = List(Map(\"\\u{D800}\",1).getOrNull(\"\\u{DFFF}\"),Map(\"\\u{D800}\",1).getOrNull(\"?\"),Map(\"\\u{D83D}\\u{DE00}\",1).getOrNull(\"\\u{1F600}\"))\n",
    "expression": "result",
    "sourceSha256": "4ce35c43dad589ff29869fec656e6eefbd82c0618e62ec7a6febb63b74f962ff",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "c4422f0a94853e47ef4e554d05e572ff7b7e9dfe2a4b45a98ade91cf50498ded",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(null, null, 1)"
  },
  {
    "id": "string-stage29-structural-key",
    "source": "result = Map(List(\"\\u{D800}\"),1,List(\"?\"),2).length\n",
    "expression": "result",
    "sourceSha256": "863435225e9b7756b62764328183e49a9bbfd3854cd15e9b891aaf13c8661829",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "2"
  },
  {
    "id": "string-stage29-literal-type-error",
    "source": "result: \"\\u{D800}\" = \"?\"\n",
    "expression": "result",
    "sourceSha256": "43c13ab40ae6d26362b97799108c3b6d95aa6eb67271e9b61497a6091faf25f3",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "641f05cb66a8a5bad3c4fe5c9a6fefe9bf30fe5c7e2530adc21bb710c11fc775",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\nExpected value of type `\"?\"`, but got type `String`.\nValue: \"?\"\n\n1 | result: \"\\u{D800}\" = \"?\"\n            ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result: \"\\u{D800}\" = \"?\"\n                         ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "string-stage29-throw",
    "source": "result = throw(\"\\u{D800}\")\n",
    "expression": "result",
    "sourceSha256": "3ca4bca67e02901d8506d1c610cd7e87fe02c3debf9090039f4fc3ba085dca83",
    "captureSha256": {
      "status": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9cabb29d957f542a3bc9aaadd466f81fd8cd1b1ed71c1d7790b889b99acf69e3",
      "error-class": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591"
    },
    "expectedError": "–– Pkl Error ––\n?\n\n1 | result = throw(\"\\u{D800}\")\n             ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "string-stage29-raw-high",
    "source": "result = ##\"\\##u{D800}\"##\n",
    "expression": "result",
    "sourceSha256": "c6a8ef7914e6d3ff28356783e3a8768fa0767a899f019f078ee8e047cd3a8e87",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "8a8de823d5ed3e12746a62ef169bcf372be0ca44f0a1236abc35df05d96928e1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "?"
  },
  {
    "id": "string-stage29-multiline-high",
    "source": "result = \"\"\"\n\\u{D800}\n\"\"\"\n",
    "expression": "result",
    "sourceSha256": "d6ea6827b1ae3d692bba742f98d68e5443ea0c0f73c58d1e996b30ec4a0193f9",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "8a8de823d5ed3e12746a62ef169bcf372be0ca44f0a1236abc35df05d96928e1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "?"
  },
  {
    "id": "string-stage29-alias-default-high",
    "source": "typealias Choice = \"\\u{D800}\"\nx: Choice\nresult = List(x==\"\\u{D800}\",x==\"?\",x.length)\n",
    "expression": "result",
    "sourceSha256": "b6d2df2ad844d6f8f0782faf8c6c342b615417cd63f6033fd511fe440f6357a6",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "85eeb931cfb57780a3fb7ee0a9db3f193a66680ca9ac335b7fb2853dc7b9b7fe",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(true, false, 1)"
  },
  {
    "id": "string-stage29-union-default-pair",
    "source": "typealias Choice = \"other\"|*\"\\u{D83D}\\u{DE00}\"\nx: Choice\nresult = x==\"\\u{1F600}\"\n",
    "expression": "result",
    "sourceSha256": "69c970c1b3f0c2fd762f68af449490e296e91fe13d6d2ff547818e6e1efce3d8",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "true"
  },
  {
    "id": "string-stage29-list-tostring-identity",
    "source": "result = List(List(\"\\u{D800}\").toString()==\"List(\\\"\\u{D800}\\\")\",List(\"\\u{D800}\").toString()==\"List(\\\"?\\\")\")\n",
    "expression": "result",
    "sourceSha256": "5a1790b9a18def2a8cbf3072a49d53311f3ff77e6e575b380e551cefc17d0c4f",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "02ce04240c0b52740b8b68612367efe8b4e4ace5364ca26e400a456d315559db",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(true, false)"
  },
  {
    "id": "string-stage29-map-tostring-identity",
    "source": "result = Map(\"\\u{D800}\",1).toString()==\"Map(\\\"\\u{D800}\\\", 1)\"\n",
    "expression": "result",
    "sourceSha256": "c22ca07bbe52b7da7bfbb3fa8da485f5489cbbbe6b81617c746ba48e9e3544bb",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "true"
  },
  {
    "id": "string-stage29-set-tostring-identity",
    "source": "result = Set(\"\\u{D800}\").toString()==\"Set(\\\"\\u{D800}\\\")\"\n",
    "expression": "result",
    "sourceSha256": "b51d951fa4d68aa7ce52fbde672b45cae24b75567b4026179b5d87fa9943e97c",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "true"
  },
  {
    "id": "string-stage29-hash-collision-map",
    "source": "result = List(Map(\"\\u{D800}\",1,\"\\u{6F0}\\u{F0}\",2).length,Map(\"\\u{D800}\",1,\"\\u{6F0}\\u{F0}\",2)[\"\\u{D800}\"],Map(\"\\u{D800}\",1,\"\\u{6F0}\\u{F0}\",2)[\"\\u{6F0}\\u{F0}\"])\n",
    "expression": "result",
    "sourceSha256": "ded2f2d9413b8625d6ce622a2c3cbae384229dd2d725045bbf958d35557b36c6",
    "captureSha256": {
      "status": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
      "stdout": "1e6a7fa5e2dc778cb1a5015e8105d4ab07973e1c79f5251b394ef133e44ea699",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "error-class": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "expectedValue": "List(2, 1, 2)"
  }
];

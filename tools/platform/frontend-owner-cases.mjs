// Complete fresh pinned Java 0.32.0 output at the identical module URI.
// Each evaluateLanguage call creates a fresh evaluator; these are frontend rejection controls.
export const frontendOwnerContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false,
  "oracle": {
    "acquisitions": {
      "target/m2/stage32-annotation-oracle-01": {
        "contextSha256": "064836920a3c5089197317276fd3e2ea19a8b6c35330a19e7ccf6bea642ab6ec",
        "reportSha256": "7e8592a72c1e21ddc0500c24c784071b746d9781a4bfa2e772a00cd41640d557"
      },
      "target/m2/stage32-annotation-oracle-04": {
        "contextSha256": "064836920a3c5089197317276fd3e2ea19a8b6c35330a19e7ccf6bea642ab6ec",
        "reportSha256": "79b18503f587a29aec53460ee1d0231cde64e04ea42dbe5cd458211e2fdd0cbd"
      },
      "target/m2/stage32-annotation-oracle-02": {
        "contextSha256": "064836920a3c5089197317276fd3e2ea19a8b6c35330a19e7ccf6bea642ab6ec",
        "reportSha256": "c0b8e8321192a71665ae610f5e001972afa31a42a04e585f62f8eaef354dc0f1"
      }
    },
    "payloadSha256": "d5f5c865753745cd6c508a7b0e15a0d8a492e197cb7679f2a8e32452b5d80e40",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf"
  }
};

export const frontendOwnerCases = [
  {
    "id": "diagnostic-stage32-object-public-negative-int",
    "source": "typealias I = Int(this > 0)\nx = new { bad: I = -1 }\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "2f2cd9953d1061de0376efb043c2fdef42c8112ca770cd0ea4c2547926ab4310",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/object-public-negative-int",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "9fe021cc03a03347ff45c21f89c30e9077ca6a4734420844d82059ba59436d33",
      "process": "a641cfa52bcfc33e3c09461172daca17192ecc2d3b2b6e50118c0044b18ab922"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n2 | x = new { bad: I = -1 }\n                   ^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-object-public-negative-float",
    "source": "typealias I = Int(this > 0)\nx = new { bad: I = -1.0 }\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "30871201f4440c8cefbbdafcd53cd7678083fa7b83a3bf4079c649e8f98b8ac5",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/object-public-negative-float",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7457c6bf957d5ac040a0d25656b5e579e96b002fc65976b390a93cc0b5bcbfd1",
      "process": "f760c6a853021c0f11641d5df5cb6da29ebe4057415e80ce525b874ca03ed2e4"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n2 | x = new { bad: I = -1.0 }\n                   ^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-object-public-grouped-negative",
    "source": "typealias I = Int(this > 0)\nx = new { bad: I = (-1) }\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "e9e84790f45fad791afb2031871f999c679e25f6a1ef3c0531ac6cc6f9f203ae",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/object-public-grouped-negative",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f94bfe14ae0bd592000a678ba8457006b28ccf3337c2ba839debfdff01e6afad",
      "process": "c9dd8fff4b3d5da0bf0a8c570cd9f0f433c899b6c0e01b982bc5cbc2c3e6f9dc"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n2 | x = new { bad: I = (-1) }\n                   ^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-object-public-negative-grouped",
    "source": "typealias I = Int(this > 0)\nx = new { bad: I = -(1) }\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "0ff8669941fa553fe00277cff88b1ed573cb57f841012f44aba98de994e9d8d3",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/object-public-negative-grouped",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5e37fa7e45e0a8654b99e438c9e68f266f7793cf1453b455ace06190f9b9098b",
      "process": "f702d292c6597045e47b1b0697ae22f6d8ff2c1c08b23bfa87ffb15099f1a8fb"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n2 | x = new { bad: I = -(1) }\n                   ^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-nested-property",
    "source": "x = new { middle = new { bad: Int = 1 } }\nresult = x.middle.bad\n",
    "expression": "result",
    "sourceSha256": "7c7ef91b33c90fe23014b9fed292af412f7f76aa0deadb1dc4cc01fb5329a939",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/nested-property",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "8aec75f8b8818268b263d1dad2f1a1338516653b51aea61350ea6e642efe11dd",
      "process": "4933a74421b4f48734634b3e1c7e565de3b5acedfca1acc08922c15a3fc11614"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { middle = new { bad: Int = 1 } }\n                                  ^^^\nat m2-platform#x.middle.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-module-function",
    "source": "function build() = new Dynamic { bad: Int = 1 }\nresult = build().bad\n",
    "expression": "result",
    "sourceSha256": "96acf0d1cd1b7fadf9cd5077315f0d80da70c43971e1f9ce1ceea344ea057183",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-04/cases/typed-module-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d28f58887dbb60ea7f6d2c40f60ab6b62a97fc9842bb39becd6aeaf19ed17bb6",
      "process": "368f6b65bbecd4137b9d08f3647cb444e6cbbdaa8afc5686558e44c8c88fd472"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function build() = new Dynamic { bad: Int = 1 }\n                                          ^^^\nat m2-platform#build.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-entry-body",
    "source": "x = new { [\"key\"] = new { bad: Int = 1 } }\nresult = x[\"key\"].bad\n",
    "expression": "result",
    "sourceSha256": "8a892a511bff18f305b0e6431b21058fa2dc90af554537a723d7b55b042571fb",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/entry-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "7327b6096cd9ff4c26fd6a014f74386b4b24d01ca33e3759383937cdf840a6da",
      "process": "7f0510795b4a750ba6042156308ebf0bbf98fe5225dffecf57cc0a0c26e0ca82"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { [\"key\"] = new { bad: Int = 1 } }\n                                   ^^^\nat m2-platform#x[\"key\"].bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-entry-astral",
    "source": "x = new { [\"\ud83d\ude00\"] = new { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "291e329e5db57ae981557a00e320ab190990101d68edbab051f99912c96d5990",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/entry-astral",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3f462347a3685897eed1318ede4dc5b4eacdcc84e499e1dfba724f9532df5d59",
      "process": "4350d45c77f2c247de614ece125b2b76b2426e53e20f75bb9b394566fa5c3128"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { [\"\ud83d\ude00\"] = new { bad: Int = 1 } }\n                                  ^^^\nat m2-platform#x[\"\ud83d\ude00\"].bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-element-body",
    "source": "x = new { new { bad: Int = 1 } }\nresult = x[0].bad\n",
    "expression": "result",
    "sourceSha256": "b221d29be0f665b4ad44cb4d4a06e98718ce6e39139ce6c4732a605b2ca0550f",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/element-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4f7b714c4dc634bf476168ac7a2dd3501622c81b739c4ec8a18f636100a4388d",
      "process": "96312d007e56116b5e1a97b247bb315c2b160ed8f3bc00ccb69eda43954f90fb"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { new { bad: Int = 1 } }\n                         ^^^\nat m2-platform#x[#1].bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-element-second",
    "source": "x = new { 0; new { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "35efa5c8ece896895360b6710cb4701d41c6492e319c4fa8f729aa13a69db740",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/element-second",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b1ca232202490295712f56c1ac1c512ca5c2d2b1de54e9b2473f8621e097b04d",
      "process": "9b38ae259cb4ee349e689c7f4852dd85431162c0aa848b20a1e0aa5125ec905f"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { 0; new { bad: Int = 1 } }\n                            ^^^\nat m2-platform#x[#2].bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-when-body",
    "source": "x = new { when (true) { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "960c80a0d0aadac2bfad5ac049d3b22edfc5925f4135c2e722d1345a200af8e5",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/when-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a04402fa7ac41747ca6ef8332d56bdd77d38dd78fe081bd960190d86d95341a9",
      "process": "b2beb815b51f1877248de05e6b8f7ebb8c91b6b980545c18010975172953517a"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { when (true) { bad: Int = 1 } }\n                                 ^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-when-false-body",
    "source": "x = new { when (false) { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "15f2555dc593241faac265ac34207e8e529f38ccc66e05efdbd076052e22bf8a",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/when-false-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1575676b93ebdbe33b26c454891329fae0642ec889189b9d40613471d2732afd",
      "process": "06ae4641050b6659196c8ce92a3a4c9f00031a960617ed86705e18ab1431cacd"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { when (false) { bad: Int = 1 } }\n                                  ^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-amendment",
    "source": "base = new { bad = 1 }\nx = (base) { bad: Int = 2 }\nresult = x.bad\n",
    "expression": "result",
    "sourceSha256": "63712e8da2de6bb48dd2a10e52d1592fc3a50049299c6c6c30d737d7f39d296e",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/amendment",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "dc4c3a29a5e8b9a46b7ee32d27be36614fef7a4e9156c4fe4cae1c83fcdc95b6",
      "process": "83831c80b81a3e7e9b8cebb090f31a26b41980d1c3672cd4e452f4bbdc3f8ba0"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n2 | x = (base) { bad: Int = 2 }\n                      ^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-nested-amendment",
    "source": "base = new { middle = new { bad = 1 } }\nx = (base) { middle { bad: Int = 2 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "d82c2debd463a8813734adc9866f9da828bbc8f07573b93a3b7146afa33e300d",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/nested-amendment",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "fbb48f4bea33a70e5da7a6390ad4dd27c9bb9c3633cea20e180c5fcdd5ff7093",
      "process": "eec90639856078d9d5433b2e5184a8b96a996007121016724b1a230d78b869d1"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n2 | x = (base) { middle { bad: Int = 2 } }\n                               ^^^\nat m2-platform#x.middle.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-let-body",
    "source": "x = let (v = 1) new { bad: Int = v }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "6fe23b120768ea1123a961282f5d42464f0469ece2fd5ddd04156ff2f26ad2c7",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/let-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ae682f6f8083d9c53ba8ac746b5a64ffb790397f092c3afad2443467660f5e40",
      "process": "a291149d053e0b866e4b1cc540c909671860b1fd5043b73bdd8466a83da340f4"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = let (v = 1) new { bad: Int = v }\n                               ^^^\nat m2-platform#x.<let expr>.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-quoted-name",
    "source": "`outer name` = new { `bad.name`: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "7fe03ff8abe9bec8c1b488ae8f95cc40ec5167c75afcef8eb34a18d135e3b549",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/quoted-name",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "03835219226fc4a4bfe8674bcc0ceadfcc8adf13054186763dd2aaabc7fc4ade",
      "process": "a812391fb675052ea953be0fb762585d08e19d8772ffb81b9858375673a8b168"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | `outer name` = new { `bad.name`: Int = 1 }\n                                     ^^^\nat m2-platform#`outer name`.`bad.name` (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-astral-name",
    "source": "`\ud83d\ude00` = new { `\ud83d\ude80`: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "721ba57b1f24ea9c044c8b3ce7571dc162534e86600c6edb60ac57e4dfb82d38",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/astral-name",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "021358e1100d9b42dc2da92faacd4dd1a76a77ae578da8d36a85a973bb5824ef",
      "process": "000ae1b49d66113b8952c4fbead797a02124e92197d6b819a1695ee9e12d80c8"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | `\ud83d\ude00` = new { `\ud83d\ude80`: Int = 1 }\n                       ^^^\nat m2-platform#`\ud83d\ude00`.`\ud83d\ude80` (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-surrogate-name",
    "source": "`\\u{D800}` = new { `\\u{D801}`: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "3d2f2a2b9b38fbdcc5c75c1eeba8e95c3ebe889af704115db99276387cce94a4",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/surrogate-name",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6e065bdbd02bb8511266d63a1373453719c434d77423caefa4916b5eedd865e0",
      "process": "58e5ad8e6d38d155e161bfb41605a59f63dcf3adc372937923a0bc8dd705d4b0"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | `\\u{D800}` = new { `\\u{D801}`: Int = 1 }\n                                   ^^^\nat m2-platform#`\\u{D800}`.`\\u{D801}` (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-first-annotation",
    "source": "x = new { first: Int = throw(\"body\"); second: String = \"a\" }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "26066f88bd8caedc6fdbf426f8360a4d8a2df36f8a8efec1556fe8c32b12a3a1",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/first-annotation",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d79fc87bdcb93bf505263000f378d4fbc1e832dfaa34d972273008a0c8b476af",
      "process": "32c7e13e0a21e14854ad133e244d90de1e6a3c5ee33350eac73441733f50b753"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { first: Int = throw(\"body\"); second: String = \"a\" }\n                     ^^^\nat m2-platform#x.first (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-annotation-before-type",
    "source": "x = new { bad: Missing = throw(\"body\") }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "133fd87623a133bf41ac2772471aae5bef3e2dfa2709ee5f13a30b9c85f63be2",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/annotation-before-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "69fd1e071db2278b1c64c57cf9184b2454ea335a4651a857f10d717f234204db",
      "process": "be20526adfbab0e2ab1dbb8db26c9b9ea01244fada2d4504e3a41f71f3b752ff"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { bad: Missing = throw(\"body\") }\n                   ^^^^^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-unused-invalid-body",
    "source": "unused = new { bad: Int = throw(\"body\") }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "75db5266961548ddf4abe7ff432e346125aba336262171aa898fcfb402eeb11d",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/unused-invalid-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "cba392f416cfd4d9b4e4ed2c433b29bcb6f84d1871616a9fc1f32257ed2dd901",
      "process": "e51d794f34c213f4661440c90d79c063e3be681b3b8dec41a298afbe81ffa363"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | unused = new { bad: Int = throw(\"body\") }\n                        ^^^\nat m2-platform#unused.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-unused-invalid-function",
    "source": "function unused() = new Dynamic { bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "6d37828c58869b361084da2e76f5854e189febe441d513bf92d4b686f351d53b",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-04/cases/typed-unused-invalid-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "1d54c5aab0b765444f182436738cc23c6a6c21394a802454e5c57d5689f7a5f7",
      "process": "f697a2e10c10405e86becbb19f42d4036d0e4bc7aab97783c90115c7a6637cec"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | function unused() = new Dynamic { bad: Int = 1 }\n                                           ^^^\nat m2-platform#unused.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-module-control",
    "source": "x: Int = 1\nresult = x\n",
    "expression": "result",
    "sourceSha256": "bb3b95e1cbe07bc85fd0da278036547a753a445aec252d6a82fe5eab4ca68ba3",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/module-control",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "c8b3a632f692f60970ac342aec41c058f85e69779f8c3bac71109964c0c01198"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage32-class-control",
    "source": "class C { x: Int = 1 }\nresult = new C {}.x\n",
    "expression": "result",
    "sourceSha256": "e17452e6ce8d7336253e3768572fe480a5db5dec338c13a5faf4a3cab63bf922",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/class-control",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "2229bb1f1b8258fa2613a645ac28866049d7a73b5ea2e625cbf79f16ff980a63"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage32-object-local-control",
    "source": "x = new { local good: Int = 1; value = good }\nresult = x.value\n",
    "expression": "result",
    "sourceSha256": "d3605a50a87a7d1f29946006562e1097a6497dbdba84825bf47ccf8b4fbf5aaf",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/object-local-control",
    "captureSha256": {
      "stdout": "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "590d34180aa8f6099e67f92f9a67d068750ec7296fbf53405bd627482df2d5fb"
    },
    "expectedValue": "1"
  },
  {
    "id": "diagnostic-stage32-local-lazy-control",
    "source": "x = new { local unused: Int = throw(\"unused\"); value = 2 }\nresult = x.value\n",
    "expression": "result",
    "sourceSha256": "209fbf13dc08d4a0e7513f7c153891a4ae4f5a57da696e15f4bdf8ff787aec10",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/local-lazy-control",
    "captureSha256": {
      "stdout": "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "process": "b0c283b867cc17bcf45e5c80bd5fa20fd7a34f9b2970adfc663d9b459442481a"
    },
    "expectedValue": "2"
  },
  {
    "id": "diagnostic-stage32-hidden-annotation",
    "source": "x = new { hidden bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "603348ebc44e1e3bf6ba0a6de4c503291f1bd3036ed1e7f0016dea611fb1b99e",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/hidden-annotation",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e562e8315442020faecd36239daee552ff4303f8f161b347b9172641b6c0a61c",
      "process": "f407a126ccb1c3b88f205161416b896b056db6080778a18d54354e93a0f1ff1f"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nModifier `hidden` is not applicable to object members.\n\n1 | x = new { hidden bad: Int = 1 }\n              ^^^^^^\nat m2-platform#x (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-nested-three",
    "source": "x = new { middle = new { inner = new { bad: Int = 1 } } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "a6941e6716ef83af83be0aeb43aabdda2e450dc257dc026752015c455a0831b8",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-01/cases/nested-three",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "97af0ce9080f8f5864a73d8b3270c772baa9708742da1d32a028f45481e7b3a5",
      "process": "aeb867bc06fe78f2087cc661dbc48897c9eee1fad79b3eb6f1154720f246f5b0"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { middle = new { inner = new { bad: Int = 1 } } }\n                                                ^^^\nat m2-platform#x.middle.inner.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-object-local-function",
    "source": "x = new { local function build() = new Dynamic { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "a07805c69cd055c3fd81407f96cedddddc7f69b0ce7ef28d064eb338b4165dc0",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-04/cases/typed-object-local-function",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ea81a81eaa9266030a1f95f2bc46f86a82e9c4ec9bb82cb20f941347482759fd",
      "process": "a5e72aafe472008dbd0a4b82265e64b6d5ad0aabd585ce1cead6c6aacee8337d"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { local function build() = new Dynamic { bad: Int = 1 } }\n                                                          ^^^\nat m2-platform#x.build.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-typed-lambda",
    "source": "f = () -> new Dynamic { bad: Int = 1 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "ab24b9e08aeea78a32fb1ad77de440aa028b0fd450f0f49dce907b6d72024d3d",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/typed-lambda",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "515e7cf8a53e7ecc0a23e771eff887df39fedd7c08d56f2afae96e77d1f6bb30",
      "process": "ffd1d179ad47c17234e35919e913d140025174d5349819fa7a60a4c37fb58bcd"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | f = () -> new Dynamic { bad: Int = 1 }\n                                 ^^^\nat m2-platform#f.<function#1>.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-typed-nested-lambda",
    "source": "f = () -> (() -> new Dynamic { bad: Int = 1 })\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "9cc0798c3e39f230e773c86e56828388fb833f42aa7d588fd56ec414c559bd23",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/typed-nested-lambda",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "df5895c176dcafcb979124e58393fa3d7253d4bf393e5d80c4645d27e3cf57fe",
      "process": "54f8a1221a3477d17a0917c6772711bfdcfbefcbbaeef5820053292eadd2b9c6"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | f = () -> (() -> new Dynamic { bad: Int = 1 })\n                                        ^^^\nat m2-platform#f.<function#2>.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-typed-entry-key",
    "source": "x = new { [new Dynamic { bad: Int = 1 }] = 2 }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "8ae37ffe15d9f2df6ae405f7386048fd1b5eda30656b124e21bbb57e2a4dc5e8",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/typed-entry-key",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a38a55dea0c9ed8da65822dededb0057bcdaf73ce93e6c5bbc0fdb6d81760bba",
      "process": "61c8136aa763786f0b931c5e2949e37b569583657b9ab5bf92233655f23a5838"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { [new Dynamic { bad: Int = 1 }] = 2 }\n                                  ^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-for-entry-body",
    "source": "x = new { for (v in List(1)) { [v] = new Dynamic { bad: Int = v } } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "7069d29fa86e2cbaf0d1c6f8b77bce272a08cdbe28a76bd9029fffb3c511fe58",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/for-entry-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "2b6a0c17e0c04d5181a600d8dc6f5872b4eed0e223dc2eb850edf8c1865dd482",
      "process": "81db1c8d0c583465a305f3eadec03f12f3f8832605f54d9a023066af39e33e5f"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { for (v in List(1)) { [v] = new Dynamic { bad: Int = v } } }\n                                                            ^^^\nat m2-platform#x[#1].bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-typed-let-initializer",
    "source": "x = let (v = new Dynamic { bad: Int = 1 }) v\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "5b13cfd4bb5cb6ab0ec149c055434480eaaa2a937426d2d151f25d98ec9d0b51",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/typed-let-initializer",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "82bfc3687eaf96985b3f6973460dda88b52bf2eb696a7a6c121a10748e5dcc94",
      "process": "e4052f220da6b40f5c07344924b66f443e4f57fb0c99ad9105ff919f017878ad"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = let (v = new Dynamic { bad: Int = 1 }) v\n                                    ^^^\nat m2-platform#x.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-typed-type-predicate",
    "source": "typealias T = Int((new Dynamic { bad: Int = 1 }).bad == this)\nresult: T = 1\n",
    "expression": "result",
    "sourceSha256": "4e2e1a07d0064e08a2e5db9f736aacef0244108ae3fc3de254e7cfab50809c76",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/typed-type-predicate",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a385ef7d70f4c71e01509ac437376a666ba1db74366b9ebced7718fd587d5df3",
      "process": "95b6ecd633c69b7096b1f5954d777e4242b95385ff15baa805a0f0a229524a76"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | typealias T = Int((new Dynamic { bad: Int = 1 }).bad == this)\n                                          ^^^\nat m2-platform#T.bad (file:///m2-platform.pkl)\n"
  },
  {
    "id": "diagnostic-stage32-entry-surrogate",
    "source": "x = new { [\"\\u{D800}\"] = new { bad: Int = 1 } }\nresult = 1\n",
    "expression": "result",
    "sourceSha256": "08ae72ec63cadb80058555531c24bede6ebc5de6aef5233f957e76bb19d0669a",
    "oracleCasePath": "target/m2/stage32-annotation-oracle-02/cases/entry-surrogate",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "36fccf3beebc2dbdf5d164e308bb2f04ffbbbb41bb85a2dd886b8b3aa172701f",
      "process": "56621df4b23fe0a528594b839a1cab5d724da5f25118593dffb5c0176838f810"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nA non-local object property cannot have a type annotation.\n\n1 | x = new { [\"\\u{D800}\"] = new { bad: Int = 1 } }\n                                        ^^^\nat m2-platform#x[\"?\"].bad (file:///m2-platform.pkl)\n"
  }
];

// Complete payloads captured from pinned Java 0.32.0 at the identical URI.
// Every portable call starts a fresh evaluator.
export const methodParentContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false,
  "oracle": {
    "reportPath": "target/m2/stage34-property-selected-oracle-01/report.json",
    "reportSha256": "b58e64b80f6415c9e83cbca9464f3783748b9ac41b27e43a9fbe00c7ad6b6b92",
    "payloadSha256": "dae490801a9a773782002d539d35af67dbc09ed073fe4c17c9ef4851e85bf9a0",
    "version": "0.32.0",
    "artifactSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "lockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "previousAcquisition": {
      "reportPath": "target/m2/stage34-supplemental-oracle-01/report.json",
      "reportSha256": "71118be38d93f4bd67db7d7228b5815db99bcd5f89f3189bd6ac05c169ffe2e5",
      "payloadSha256": "83649a5c024dcc62ca7595fd223416ae6cd346e90ad1bc693c05ab0bf5301eef",
      "version": "0.32.0",
      "artifactSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
      "lockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
      "previousAcquisition": {
        "reportPath": "target/m2/stage34-selected-oracle-01/report.json",
        "reportSha256": "cb101f3d70aa304880a2cf53a97aeccf67b1bb0a3997f8a353bec5f8910a96f9",
        "payloadSha256": "0d2b3d1be67c9199cc15411644cfba22276bc167722758462e7d00401b094ef4",
        "version": "0.32.0",
        "artifactSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
        "lockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b"
      }
    }
  }
};

export const methodParentCases = [
  {
    "id": "method-parent-stage34-abstract-class",
    "source": "abstract class Person { age: Int = 7 }\nfunction build(): Person = new {}\nresult = build().age.toString()\n",
    "sourceSha256": "78df80320989b6cf7bd62369aa3f8ab671df6671fea1eb7171bd48a8bd27d04c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-abstract-class",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e4c0682b8b92adf274a5b93fe667fe690fd66e2321eedc425ade28adbafc9e4b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function build(): Person = new {}\n                               ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n3 | result = build().age.toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-alias",
    "source": "class Person { age: Int = 7 }\ntypealias Alias = Person\nfunction build(): Alias = new { age = 9 }\nresult = build().age.toString()\n",
    "sourceSha256": "7a0001d7cb2b0584058c30d1bf9a25ccfb9c7d53c1b478542211f1dfb3b61b8b",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-alias",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-any",
    "source": "function build(): Any = new { value = 5 }\nresult = build().value.toString()\n",
    "sourceSha256": "098506716a9f1ee8cd9708058eae777cc08f4935a23efe5aeb6fd26ef435968a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-any",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "69f0e8558bae682979fbfaaece0031e8258afad033d1ab1516d13a7df42e2a5b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | function build(): Any = new { value = 5 }\n                            ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n2 | result = build().value.toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-class-default",
    "source": "class Person { name: String = \"default\"; age: Int = 7 }\nfunction build(): Person = new { name = \"new\" }\nresult = build().name + \":\" + build().age.toString()\n",
    "sourceSha256": "480a1f60de7e6a6023afa75e50c79f2a22f2ad5bcc38d5d20bf05fbabd18d99a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-class-default",
    "captureSha256": {
      "stdout": "b00c0e0aef5e8d7e339bdc58bee194ae87c339e684327067ceb45865b5df4d12",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "new:7"
  },
  {
    "id": "method-parent-stage34-class-inherited",
    "source": "open class Base { a: Int = 3; b: Int = 4 }\nclass Child extends Base { a = 5 }\nfunction build(): Child = new { b = 9 }\nresult = build().a.toString() + \":\" + build().b.toString()\n",
    "sourceSha256": "2fecbfe576fe5e1d5811f68affbd51d45e6ca7e6b3285fb3962fad15bd5ebfaa",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-class-inherited",
    "captureSha256": {
      "stdout": "e0e9d38f9ccb7a41e441488bd71183266da0aaa606b7bf344ab42fb9c0a70dd1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "5:9"
  },
  {
    "id": "method-parent-stage34-class-method",
    "source": "class Person { age: Int = 7 }\nclass Factory { function build(): Person = new { age = 9 } }\nresult = (new Factory {}).build().age.toString()\n",
    "sourceSha256": "0c2dcb20f77cb50c6f86ec759776a7003472038ab698477582148ee6446f2b72",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-class-method",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-dynamic",
    "source": "function build(): Dynamic = new { value = 5 }\nresult = build().value.toString()\n",
    "sourceSha256": "464ff275b71d0b94c3f7228507a2c43e4cbb77f2894af0e3cb4a1711c3c68b32",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-dynamic",
    "captureSha256": {
      "stdout": "ef2d127de37b942baad06145e54b0c619a1f22327b2ebbcfbec78f5564afe39d",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "5"
  },
  {
    "id": "method-parent-stage34-first-argument-before-default",
    "source": "function build(n: Int): Int = new {}\nresult = build(throw(\"argument\")).toString()\n",
    "sourceSha256": "769c6a8ef2c0e0170f70f5d2955ff592a266027b938d006c9df74c37ade5ede9",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-first-argument-before-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "fd53e050e20037353b7d6df6dcf6608a746ca61ad6724529b9a3d8b87ccc3753",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nargument\n\n2 | result = build(throw(\"argument\")).toString()\n                   ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-first-condition-before-default",
    "source": "function build(): Int = if (throw(\"condition\")) new {} else new {}\nresult = build().toString()\n",
    "sourceSha256": "e6ab9f0828458fbb54f86a040dc45a99e595435e426961eddd32ecae75e3f527",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-first-condition-before-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "20a1d04b987b4ebc8713503563530c0b058cc3fafbbd0fd6dd3da313e69a1856",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\ncondition\n\n1 | function build(): Int = if (throw(\"condition\")) new {} else new {}\n                                ^^^^^^^^^^^^^^^^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\n2 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-first-default-before-body",
    "source": "function build(): Int = new { value = throw(\"body\") }\nresult = build().toString()\n",
    "sourceSha256": "e0ffb79c574070131142f382b91828eeee928e7506711f62d491ed23aa3bece2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-first-default-before-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "76708c25a977e6b1e7417a528e791846277729a3b9bb73cddd6f6ef59b9da1e3",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | function build(): Int = new { value = throw(\"body\") }\n                            ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n2 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-first-let-before-default",
    "source": "function build(): Int = let (n = throw(\"binding\")) new {}\nresult = build().toString()\n",
    "sourceSha256": "0bec8c4784e5ebe167acb52c89cc63d65f9c5199a10b2e5dbf9686ab12834eee",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-first-let-before-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "2200d03f23bc1996577aaf58468b9b00c0f372bec51bbf01f075c6bd0e240038",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nbinding\n\n1 | function build(): Int = let (n = throw(\"binding\")) new {}\n                                     ^^^^^^^^^^^^^^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\n2 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-first-type-before-new",
    "source": "function build(): Missing = new { value = throw(\"body\") }\nresult = build().toString()\n",
    "sourceSha256": "6c1b9c96ec89851d5e0cf95c096577490e6d40c7ccd7ec5402f44e9366167a5a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-first-type-before-new",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c6790ce965d8f72e482c5886dc971c9cfd98c3c0908c36097e55f890175d014c",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot find type `Missing`.\n\n1 | function build(): Missing = new { value = throw(\"body\") }\n                      ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "method-parent-stage34-generic-alias",
    "source": "typealias Values<T> = Listing<T>\nfunction build(): Values<Int> = new { 3 }\nresult = build()[0].toString()\n",
    "sourceSha256": "da5fc93c24b04ab3fff5a2e415dfe2b5cb578ec086e0cf46ce2e8b04f329e877",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-generic-alias",
    "captureSha256": {
      "stdout": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "3"
  },
  {
    "id": "method-parent-stage34-generic-alias-mapping",
    "source": "typealias Values<T> = Mapping<String, T>\nfunction build(): Values<Int> = new { [\"x\"] = 4 }\nresult = build()[\"x\"].toString()\n",
    "sourceSha256": "9c4af83a81e039ea39250f2679908f26d72b27f7dc57e16975a4dd4fe8f97052",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-generic-alias-mapping",
    "captureSha256": {
      "stdout": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "4"
  },
  {
    "id": "method-parent-stage34-if-let",
    "source": "class Person { age: Int = 7 }\nfunction build(flag: Boolean): Person = if (flag) let (n = 9) new { age = n } else new {}\nresult = build(true).age.toString() + \":\" + build(false).age.toString()\n",
    "sourceSha256": "131f698ec82bbbd8002d821dd0707a3027ae8c0b7120d41f0e731cf7e937c89d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-if-let",
    "captureSha256": {
      "stdout": "384fcabee9b1d0f9193321434a7393ab0594cdace49c155a7d1bf56c26209841",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9:7"
  },
  {
    "id": "method-parent-stage34-inherited-method",
    "source": "class Person { age: Int = 7 }\nopen class Factory { function build(): Person = new { age = 9 } }\nclass Child extends Factory {}\nresult = (new Child {}).build().age.toString()\n",
    "sourceSha256": "36e7e972015678fc3c4d6a52031453f12e11c9e53db10e896c272a3954b26488",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-inherited-method",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-int-no-default",
    "source": "function build(): Int = new {}\nresult = build().toString()\n",
    "sourceSha256": "32625d652f15595db09fb3c7c916c99cf93a6ff833ccc9ff5af30d7dcfab6e7c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-int-no-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "2adb1fa5cfd5789f54c2ddd42fa7dc06929f26fa388fce0fc4abad286c51e776",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | function build(): Int = new {}\n                            ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n2 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-lambda-rejected",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person = let (f = () -> new { age = 9 }) f()\nresult = build().age.toString()\n",
    "sourceSha256": "70804f8ebedba5e39dea0088ff45c14c6f42a04269b0f4182a7bc70beb2c9088",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-lambda-rejected",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "ce130f3a7a84214b39c48cf5e13c530d4a503d0c784a0b1cdb2de7e5e0e4e7ec",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function build(): Person = let (f = () -> new { age = 9 }) f()\n                                              ^^^\nat m2-platform#build.<function#1> (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "method-parent-stage34-lazy-return-validation",
    "source": "typealias Bad = Listing<Missing>\nfunction build(): Bad = if (true) throw(\"body\") else new {}\nresult = build().toString()\n",
    "sourceSha256": "ba914dad7be7b591e6ba31b41bfcc2d6785b7f7ef8ef98a82380f13374d04060",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-lazy-return-validation",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "19e3710599b8e4c13272790fc22ca8e0b121e8538873aeb8a778d3f4280a5c1d",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot find type `Missing`.\n\n1 | typealias Bad = Listing<Missing>\n                            ^^^^^^^\nat m2-platform#Bad (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n\n2 | function build(): Bad = if (true) throw(\"body\") else new {}\n                      ^^^\nat m2-platform (file:///m2-platform.pkl)\n"
  },
  {
    "id": "method-parent-stage34-lazy-skipped-new",
    "source": "function build(): Any = if (false) new {} else \"ok\"\nresult = build().toString()\n",
    "sourceSha256": "a4358a8f45ce2d23f6ffca815c7d856df41d62ba81acce58423e80b599314c89",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-lazy-skipped-new",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "method-parent-stage34-lazy-unused-signature",
    "source": "function unused(): Missing = new {}\nresult = \"ok\"\n",
    "sourceSha256": "7990da6a8e0793e7e6fa701cf2e2eda74b69ea21c546797d9a488f6658ac2f51",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-lazy-unused-signature",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e33ef46273464514dc5af665613e68ac58fab6a4045793c70f0cb15abc641ad5",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot find type `Missing`.\n\n1 | function unused(): Missing = new {}\n                       ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "method-parent-stage34-let-initializer-rejected",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person = let (p = new {}) p\nresult = build().age.toString()\n",
    "sourceSha256": "776df4e451cab2239f81e507c0ab5c9a7928122f580041c736e57fbf1dbda66f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-let-initializer-rejected",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "49320454f46df997badf5c7be48afbc568160c7e0cd07a9907e0bba724230d78",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function build(): Person = let (p = new {}) p\n                                        ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "method-parent-stage34-lexical-capture",
    "source": "class Person { age: Int = 7 }\nseed = 11\nfunction build(seed: Int): Person = new { age = seed }\nresult = build(9).age.toString() + \":\" + seed.toString()\n",
    "sourceSha256": "47f9565d2a0ddc3f9cc5f4032ab954d9e00b6579b7ac516c3998c39b3ea24053",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-lexical-capture",
    "captureSha256": {
      "stdout": "6901b4090b059f1e5f547902f2520d6f11133ffedac383f5a0f9cc9ed534c2a0",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9:11"
  },
  {
    "id": "method-parent-stage34-list-no-default",
    "source": "function build(): List<Int> = new {}\nresult = build().toString()\n",
    "sourceSha256": "a62d76f563b7b2ef908a6ea8165bfe3cc1039e1ed9163f62dbda0dbaea056dcf",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-list-no-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3b7fd3af6aee0ad6657a48ed6f0cd503646646081740c3ba47ed2a49337aff48",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot instantiate, or amend an instance of, external class `List`.\n\n1 | function build(): List<Int> = new {}\n                                  ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nTo create an instance of an external class, use its literal syntax or constructor method.\nExamples:\n* `123` uses literal syntax to create an instance of class `Int`.\n* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.\n\n2 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-listing",
    "source": "function build(): Listing<Int> = new { 1; 2 }\nresult = build()[0].toString() + \":\" + build()[1].toString()\n",
    "sourceSha256": "84827df86be78ed211387c5104a9f486f0c49f0b42a189d826be0d7f8d7e24b5",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-listing",
    "captureSha256": {
      "stdout": "673aeeb08cfbb00b91e5e3c60b5ba31896d55b522b1be2dccb0645a59bae8775",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1:2"
  },
  {
    "id": "method-parent-stage34-listing-value-type",
    "source": "function build(): Listing<Int> = new { \"bad\" }\nresult = build()[0].toString()\n",
    "sourceSha256": "72c967836656cdc9ef53de65538bc60bbbd69e1c0cad502cac62b8fe82f4c579",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-listing-value-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "5530db64b5919a7d97fe8f38cd96002c92375cafe1d8bd5533a0bc621004ce86",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `String`.\nValue: \"bad\"\n\n1 | function build(): Listing<Int> = new { \"bad\" }\n                              ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\n1 | function build(): Listing<Int> = new { \"bad\" }\n                                           ^^^^^\nat m2-platform#build[#1] (file:///m2-platform.pkl)\n\n2 | result = build()[0].toString()\n             ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-local-object",
    "source": "class Person { age: Int = 7 }\nx = new { local function build(): Person = new { age = 9 }; value = build().age }\nresult = x.value.toString()\n",
    "sourceSha256": "9c7331dcde7d25babb9f05cca84d71314b7bceeed471b3d4337f3fd3be790539",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-local-object",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-mapping",
    "source": "function build(): Mapping<String, Int> = new { [\"a\"] = 2 }\nresult = build()[\"a\"].toString()\n",
    "sourceSha256": "506393c91025f12d8d20ff58e49efabc4e256aa9c1ec38feb20c55cfc123cd0a",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-mapping",
    "captureSha256": {
      "stdout": "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "2"
  },
  {
    "id": "method-parent-stage34-mapping-key-type",
    "source": "function build(): Mapping<String, Int> = new { [1] = throw(\"value\") }\nresult = build().length.toString()\n",
    "sourceSha256": "16c90ea88fcc0e6913a6cc9c8389bfb147d4199d44b1296c6b350c450ee90d0b",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-mapping-key-type",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "36a014e374f0507641d28ad8d97e83ec693664faef0c82510fecb712cd9c8b77",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n1 | function build(): Mapping<String, Int> = new { [1] = throw(\"value\") }\n                              ^^^^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\n1 | function build(): Mapping<String, Int> = new { [1] = throw(\"value\") }\n                                                    ^\nat m2-platform#build[1] (file:///m2-platform.pkl)\n\n2 | result = build().length.toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-new-explicit-control",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person = new Person { age = 9 }\nresult = build().age.toString()\n",
    "sourceSha256": "818abfe1c930833cdc13a7ab2d709bfb1e61090a45cde4e2db86bec8143aca5b",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-new-explicit-control",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-new-nested-property",
    "source": "class Person { age: Int = 7 }\nclass Envelope { child: Person }\nfunction build(): Envelope = new { child = new { age = 9 } }\nresult = build().child.age.toString()\n",
    "sourceSha256": "72b6f0ab511cf4213f7e7d0e30220f32f7002987d5c85ef9648f6e97dd3b274c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-new-nested-property",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-no-annotation",
    "source": "function build() = new { value = 5 }\nresult = build().value.toString()\n",
    "sourceSha256": "aa3b7a1a978950a2b442cb8bf2b9c4aee10e9c6712af7e99529830ac2d6e62cf",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-no-annotation",
    "captureSha256": {
      "stdout": "ef2d127de37b942baad06145e54b0c619a1f22327b2ebbcfbec78f5564afe39d",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "5"
  },
  {
    "id": "method-parent-stage34-nullable-class",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person? = new { age = 9 }\nresult = build()!!.age.toString()\n",
    "sourceSha256": "5e49b6e3245705b908eecd0bc139db4ed0a95b5c3a81c757f57d442d4387e679",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nullable-class",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-nullable-empty",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person? = new {}\nresult = (build() == null).toString()\n",
    "sourceSha256": "32e14c0380d644bbf13f6f20f7c585ebd3a77c7ccac58e42047c3fa795e61216",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nullable-empty",
    "captureSha256": {
      "stdout": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "false"
  },
  {
    "id": "method-parent-stage34-nullable-int",
    "source": "function build(): Int? = new {}\nresult = (build() == null).toString()\n",
    "sourceSha256": "3784f1d3dcd5d67bcfc75cf212f34c59a8bf515da6ae2246e41d60584a5da5ed",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nullable-int",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3f4e3ef4a9a63a15eda0f47b0ed14a0b5b02c29abfb066c9e811aef62dbee7b5",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Dynamic`.\nValue: new Dynamic {}\n\n1 | function build(): Int? = new {}\n                      ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\n2 | result = (build() == null).toString()\n              ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-override-method",
    "source": "open class Person { age: Int = 7 }\nclass Child extends Person { age = 8 }\nopen class Factory { function build(): Person = new { age = 9 } }\nclass ChildFactory extends Factory { function build(): Child = new {} }\nresult = (new ChildFactory {}).build().age.toString()\n",
    "sourceSha256": "4b7c9cb5cd0ca549986dca966c1ca33994dabffd942a3e7959ee431679e7549b",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-override-method",
    "captureSha256": {
      "stdout": "2c624232cdd221771294dfbb310aca000a0df6ac8b66b696d90ef06fdefb64a3",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "8"
  },
  {
    "id": "method-parent-stage34-paren-rejected",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person = (new { age = 9 })\nresult = build().age.toString()\n",
    "sourceSha256": "718053079ca0394e6fffd55ce61bbd43ccb02d0857f87ed015bce18cf4ab7a3c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-paren-rejected",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "06ad10767bff52128c66c60a33c63850326ae7480c94e6596395525e0b6f0e4b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function build(): Person = (new { age = 9 })\n                                ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n"
  },
  {
    "id": "method-parent-stage34-receiver-capture",
    "source": "class Person { age: Int = 7 }\nclass Factory { seed = 11; function build(): Person = new { age = seed } }\nresult = (new Factory {}).build().age.toString()\n",
    "sourceSha256": "8878d2742fa3a73a3703189248c37aaeba2f0cd95be731ecdc6a86d22444e0df",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-receiver-capture",
    "captureSha256": {
      "stdout": "4fc82b26aecb47d2868c4efbe3581732a3e7cbcc6c2efb32062c08170a05eeb8",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "11"
  },
  {
    "id": "method-parent-stage34-string-default",
    "source": "function build(): String = new {}\nresult = build().toString()\n",
    "sourceSha256": "62065e58d97081efa8eae6bb80112f7a5469acc4dbaeaa3570c14ee313d5b673",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-string-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "f9182e33ef5246e948984e940529c642e5982f8d22e89a0521c2f98ef70c11ee",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n1 | function build(): String = new {}\n                               ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n2 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-union-default",
    "source": "class Person { age: Int = 7 }\nfunction build(): *Person | String = new { age = 9 }\nresult = build().age.toString()\n",
    "sourceSha256": "ffa7c0bb64378aa3c94e297f2d62b5a0a75b6e2fc46c28546530e9e1d345a144",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-union-default",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "method-parent-stage34-union-no-default",
    "source": "class Person { age: Int = 7 }\nfunction build(): Person | String = new {}\nresult = build().toString()\n",
    "sourceSha256": "9b6dd48e7e8f8e1fbffca4cae5f94a89e7ea55b07d022e665214283e81d7c2b9",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-union-no-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "b42bb7aa5c7bc998d072d6925cc8b81d84fa17a2a4e75d4a8cbcbd77bf276e3d",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function build(): Person | String = new {}\n                                        ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n3 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-unknown",
    "source": "function build(): unknown = new { value = 5 }\nresult = build().value.toString()\n",
    "sourceSha256": "567381b3e7736f3e44e3a6ee2adf23715a3b1aea6a6eba7bb38317e889d0192f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-unknown",
    "captureSha256": {
      "stdout": "ef2d127de37b942baad06145e54b0c619a1f22327b2ebbcfbec78f5564afe39d",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "5"
  },
  {
    "id": "method-parent-stage34-local-object-listing",
    "source": "x = new { local function build(): Listing<Int> = new { 6 }; value = build()[0] }\nresult = x.value.toString()\n",
    "sourceSha256": "1416596dcf53606f6ceaaf4358881e41831474354a3000ce596a3a65450cfe97",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-local-object-listing",
    "captureSha256": {
      "stdout": "e7f6c011776e8db7cd330b54174fd76f7d0216b612387a5ffcfb81e6f0919683",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "6"
  },
  {
    "id": "method-parent-stage34-local-object-mapping",
    "source": "x = new { local function build(): Mapping<String, Int> = new { [\"x\"] = 6 }; value = build()[\"x\"] }\nresult = x.value.toString()\n",
    "sourceSha256": "6c131f1ff76f7a9471cd4f05e00de0e1e43c0450df0226d6a0a3757cf7a25ee1",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-local-object-mapping",
    "captureSha256": {
      "stdout": "e7f6c011776e8db7cd330b54174fd76f7d0216b612387a5ffcfb81e6f0919683",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "6"
  },
  {
    "id": "method-parent-stage34-local-return-constraint",
    "source": "typealias Positive = Int(this > 0)\nx = new { local function build(): Positive = new {}; value = build() }\nresult = x.value.toString()\n",
    "sourceSha256": "1f4c00a0662c3a457bd64e278ab3945add17fb2d7e4f8a9c065d57d780b97aae",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-local-return-constraint",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "89104bcd2664deef6d366d571f22e156ff1af47fa298dd486f9e0ef3cb15f272",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | x = new { local function build(): Positive = new {}; value = build() }\n                                                 ^^^\nat m2-platform#x.build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n2 | x = new { local function build(): Positive = new {}; value = build() }\n                                                                 ^^^^^^^\nat m2-platform#x.value (file:///m2-platform.pkl)\n\n3 | result = x.value.toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-nested-defaults",
    "source": "class Person { age: Int = 7 }\nfunction build(): Listing<Person> = new { new {} }\nresult = build()[0].age.toString()\n",
    "sourceSha256": "7ff3d380aca4ecc784bb3ed2d0944eb3053069f0069f6b40aac79ab7940722c1",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nested-defaults",
    "captureSha256": {
      "stdout": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "7"
  },
  {
    "id": "method-parent-stage34-nested-generic-alias",
    "source": "typealias Rows<T> = Listing<Listing<T>>\nfunction build(): Rows<Int> = new { new { 3 } }\nresult = build()[0][0].toString()\n",
    "sourceSha256": "78cfc3658fed2b2f030b495b8ae0ffc153e9e56885a21b73a5dc6c2bed86edf8",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nested-generic-alias",
    "captureSha256": {
      "stdout": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "3"
  },
  {
    "id": "method-parent-stage34-nullable-listing",
    "source": "function build(): Listing<Int>? = new { 3 }\nresult = build()!![0].toString()\n",
    "sourceSha256": "6dad6b6410e08f6a2fd4e5b2fe281e61ee88a93b4c7ceb851afc1d4cb2d93c50",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nullable-listing",
    "captureSha256": {
      "stdout": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "3"
  },
  {
    "id": "method-parent-stage34-nullable-mapping",
    "source": "function build(): Mapping<String, Int>? = new { [\"x\"] = 4 }\nresult = build()!![\"x\"].toString()\n",
    "sourceSha256": "c745ac3b0dcbe052f625e8287afec3aa4e722840416ccb39a2512301532a6e47",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-nullable-mapping",
    "captureSha256": {
      "stdout": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "4"
  },
  {
    "id": "method-parent-stage34-repeated-arguments",
    "source": "class Person { age: Int = 7 }\nfunction build(n: Int): Person = new { age = n }\nresult = build(1).age.toString() + \":\" + build(2).age.toString()\n",
    "sourceSha256": "241ce17d000e6b496899888b8fa4f2858d9744cc8b0278b7a00f0f9c3dfabce2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-repeated-arguments",
    "captureSha256": {
      "stdout": "673aeeb08cfbb00b91e5e3c60b5ba31896d55b522b1be2dccb0645a59bae8775",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "1:2"
  },
  {
    "id": "method-parent-stage34-type-before-argument",
    "source": "function build(n): Missing = new {}\nresult = build(throw(\"argument\")).toString()\n",
    "sourceSha256": "dc413cf32b13f1314081982c3591ae25bc056509e5b558845d060e9e25267050",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-type-before-argument",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "a295660b736b55c41292439b2e3c4fb42e1f344c67fc7b3ba9b3aed79bbdc755",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot find type `Missing`.\n\n1 | function build(n): Missing = new {}\n                       ^^^^^^^\nat m2-platform (file:///m2-platform.pkl)\n\nSearched in the following places:\n* Module `m2-platform` and any supermodules\n* Module `pkl#base`\n"
  },
  {
    "id": "method-parent-stage34-generic-unknown-alias-element",
    "source": "typealias V<T> = Listing<T>\nfunction build(): V<unknown> = new { new {} }\nresult = build()[0].toString()\n",
    "sourceSha256": "1cfea212e1d4d86b9947eb5bb9f18d25dffd7a5e4197c0a43fe901c6edb5b422",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-generic-unknown-alias-element",
    "captureSha256": {
      "stdout": "6948d378b5db7417650c4ecf402848506a6ea9f9fd88fd0933b521f649272507",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "new Dynamic {}"
  },
  {
    "id": "method-parent-stage34-if-condition",
    "source": "class C {}\nfunction f(): C = if (new {}) new {} else new {}\nresult = f().toString()\n",
    "sourceSha256": "f167d84cacedf0a898a5d51a774ad05ec2d83a38164ba1c10b46b9f15bc0e0c2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-if-condition",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "361079f70b617e3ea9d84ad918e927b70083dd4afbe520076e8b096703340c3b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Boolean`, but got type `m2-platform#C`.\nValue: new C {}\n\n2 | function f(): C = if (new {}) new {} else new {}\n                          ^^^^^^\nat m2-platform#f (file:///m2-platform.pkl)\n\n3 | result = f().toString()\n             ^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-if-condition-unused",
    "source": "class C {}\nfunction f(): C = if (new {}) new {} else new {}\nresult = \"ok\"\n",
    "sourceSha256": "753a0a78c0e5e39f6a6b4df21a07b857a2c31dbfb8c1aea6ab6197f8b6a501ed",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-if-condition-unused",
    "captureSha256": {
      "stdout": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ok"
  },
  {
    "id": "method-parent-stage34-listing-any-element",
    "source": "function build(): Listing<Any> = new { new {} }\nresult = build()[0].toString()\n",
    "sourceSha256": "5b55a5b0cb9869b9b86e8acd6778af57d314c59553cdd86a7956300afeac438d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-listing-any-element",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "870b903f3479f2510b72ef7dbd0e0d77a40ef07a4693058fb79028b482f5c14b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `default` but its value is undefined.\n\n1 | function build(): Listing<Any> = new { new {} }\n    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#build.default (file:///m2-platform.pkl)\n\n1 | function build(): Listing<Any> = new { new {} }\n                                           ^^^\nat m2-platform#build[#1] (file:///m2-platform.pkl)\n\n2 | result = build()[0].toString()\n             ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-listing-unknown-alias-element",
    "source": "typealias U = unknown\nfunction build(): Listing<U> = new { new {} }\nresult = build()[0].toString()\n",
    "sourceSha256": "9d505b053bed77523c6d3c18b6981360c4281770f7a0719d9438e1812a539ea7",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-listing-unknown-alias-element",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "38ef63c86e810a99e3de7555ce0d3cde010803c63cbf68a2ac70f73f3a4d3aa9",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `default` but its value is undefined.\n\n2 | function build(): Listing<U> = new { new {} }\n    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#build.default (file:///m2-platform.pkl)\n\n2 | function build(): Listing<U> = new { new {} }\n                                         ^^^\nat m2-platform#build[#1] (file:///m2-platform.pkl)\n\n3 | result = build()[0].toString()\n             ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-listing-unknown-element",
    "source": "function build(): Listing<unknown> = new { new {} }\nresult = build()[0].toString()\n",
    "sourceSha256": "f7a217d533fb8df8b5dc41e26c93f05e87e6c5e9e004dc265552686d09cc46fb",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-listing-unknown-element",
    "captureSha256": {
      "stdout": "6948d378b5db7417650c4ecf402848506a6ea9f9fd88fd0933b521f649272507",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "new Dynamic {}"
  },
  {
    "id": "method-parent-stage34-mapping-any-element",
    "source": "function build(): Mapping<String, Any> = new { [\"x\"] = new {} }\nresult = build()[\"x\"].toString()\n",
    "sourceSha256": "7000afd4bf679597a7b851107e6b3cf8d29d7f748034c66ec32ae4b1d67c9f8e",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-mapping-any-element",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "25d086fa472968d8e401c8af06919d1f79aad0bac3a1919c14959b3da521d57c",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `default` but its value is undefined.\n\n1 | function build(): Mapping<String, Any> = new { [\"x\"] = new {} }\n    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#build.default (file:///m2-platform.pkl)\n\n1 | function build(): Mapping<String, Any> = new { [\"x\"] = new {} }\n                                                           ^^^\nat m2-platform#build[\"x\"] (file:///m2-platform.pkl)\n\n2 | result = build()[\"x\"].toString()\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-unknown-alias",
    "source": "typealias U = unknown\nfunction build(): U = new {}\nresult = build().toString()\n",
    "sourceSha256": "f5d5779e008f863e02f5110ff053f896ad04458cf139d0fb6f14bd0a3a52886e",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-selected-oracle-01/cases/method-parent-stage34-unknown-alias",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d94e260b32ced396a1fc8484c18efbf013d4f0c5a38e46d7576ad36d9325a727",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function build(): U = new {}\n                          ^^^\nat m2-platform#build (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n3 | result = build().toString()\n             ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "method-parent-stage34-listing-unknown-element-type-control",
    "source": "function build(): Listing<unknown> = new { new {} }\nresult = (build()[0] is Dynamic).toString()\n",
    "sourceSha256": "f26a790d3cc0ef0217e6ae946aeb851a3d9f4f3dda64f48f7cc39822b74685fd",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-supplemental-oracle-01/cases/method-parent-stage34-listing-unknown-element-type-control",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "method-parent-stage34-generic-unknown-alias-element-type-control",
    "source": "typealias V<T> = Listing<T>\nfunction build(): V<unknown> = new { new {} }\nresult = (build()[0] is Dynamic).toString()\n",
    "sourceSha256": "02744a6160420f14a0a9aff579d2ac2476d16618d6e7b261113f19eacb6a1863",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-supplemental-oracle-01/cases/method-parent-stage34-generic-unknown-alias-element-type-control",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "property-parent-stage34-explicit-super-missing-amended",
    "source": "base = new Dynamic {}\nx = (base) { value = super.missing }\nresult = (x.value is Dynamic).toString()\n",
    "sourceSha256": "64595e5a7900868a11d82fb183b712b025bbf1e07d5f823b0597af6001f3fefe",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-explicit-super-missing-amended",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "property-parent-stage34-explicit-super-missing-new",
    "source": "x = new Dynamic { value = super.missing }\nresult = (x.value is Dynamic).toString()\n",
    "sourceSha256": "fb3c9ce569de0eeeab106c62e79b7280a72b398ad1fb0738dd88e319a6f23897",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-explicit-super-missing-new",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "property-parent-stage34-explicit-super-missing-present-parent",
    "source": "base = new Dynamic { existing = 1 }\nx = (base) { value = super.missing }\nresult = (x.value is Dynamic).toString()\n",
    "sourceSha256": "5f99901642c679faba6224abd344549fb25d186cfc50aa703324200ed3552fd3",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-explicit-super-missing-present-parent",
    "captureSha256": {
      "stdout": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true"
  },
  {
    "id": "property-parent-stage34-instance-inherited-initializer",
    "source": "class C { x: Int = 7 }\nopen class A { p: C = new { x = 9 } }\nclass B extends A {}\nresult = (new B { p = new {} }).p.x.toString()\n",
    "sourceSha256": "b88d874a70dc981c13b66fc8d2c55fd10ce704f22b2917f5aaf8668389ec71fc",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-instance-inherited-initializer",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "property-parent-stage34-instance-prototype-initializer",
    "source": "class C { x: Int = 7 }\nclass A { p: C = new { x = 9 } }\nresult = (new A { p = new {} }).p.x.toString()\n",
    "sourceSha256": "8a6dd99234e150e463a9f6d45160a0db859ce1cf36a0d4dc63df3c7178405aa4",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-instance-prototype-initializer",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "property-parent-stage34-nested-abstract-default",
    "source": "abstract class Child { x: Int = 7 }\nclass C { child: Child }\nfunction make(): C = new { child = new {} }\nresult = make().child.x.toString()\n",
    "sourceSha256": "9698e766896981ca9d51bb67d43576f366a151e8b58edaa97ee38b48f75420bd",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-abstract-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "697cd410a3e33c6fcd87f308fa0adc36572cd96eda3d0f59567eb0be075beeea",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n3 | function make(): C = new { child = new {} }\n                                       ^^^\nat m2-platform#make.child (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n4 | result = make().child.x.toString()\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "property-parent-stage34-nested-constraints-skipped",
    "source": "class C { x: Int = 7 }\nclass D { child: C(this.x == 9) = new { x = 7 } }\nfunction make(): D = new { child = new { x = 9 } }\nresult = make().child.x.toString()\n",
    "sourceSha256": "b1f2f7e986e9a56b219b7fbe2def0fc9deb92e7658c3a065483b8bf27337a3b6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-constraints-skipped",
    "captureSha256": {
      "stdout": "19581e27de7ced00ff1ce50b2047e7a567c76b1cbaebabe5ef03f7c3017bb5b7",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "9"
  },
  {
    "id": "property-parent-stage34-nested-default-field-undefined",
    "source": "class Child { x: Int }\nclass C { child: Child = new {} }\nfunction make(): C = new { child = new {} }\nresult = make().child.x.toString()\n",
    "sourceSha256": "f104952db8cfd56ccbb5c7c042df5ff48318cdc1489de142444fd5eed1f6e348",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-default-field-undefined",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c58a2983ec91d15088313867532ee5191b3d1af82c3736e6f26aeccdf2d9e25b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `x` but its value is undefined.\n\n1 | class Child { x: Int }\n                  ^\nat m2-platform#Child.x (file:///m2-platform.pkl)\n\n4 | result = make().child.x.toString()\n             ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "property-parent-stage34-nested-default-throw-before-body",
    "source": "class Child { x: Int = 7 }\nclass C { child: Child = throw(\"default first\") }\nfunction make(): C = new { child = new { x = throw(\"body second\") } }\nresult = make().child.x.toString()\nstable = 23\n",
    "sourceSha256": "b4cf6f0a7ffbc7896e4f77d0ff4539b450a52fd576c78898630cd247cbec2ccb",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-default-throw-before-body",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "88e9b4a94bcccc2935340c3535562ee0fd2ea833bdfa15ba8f42da070d74093b",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\ndefault first\n\n2 | class C { child: Child = throw(\"default first\") }\n                             ^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#C.child (file:///m2-platform.pkl)\n\n4 | result = make().child.x.toString()\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "property-parent-stage34-nested-if-let-super",
    "source": "class C { x: Int = 7 }\nclass D { child: C = new { x = 12 } }\nfunction make(): D = new { child = if (true) let (v = 3) new { x = super.x + v } else new {} }\nresult = make().child.x.toString()\n",
    "sourceSha256": "43709a87dd509b367c6f72ae66a5ba1a3dbb53621d947d68b3a886428ba66b38",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-if-let-super",
    "captureSha256": {
      "stdout": "e629fa6598d732768f7c726b4b621285f9c3b85303900aa912017db7617d8bdb",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "15"
  },
  {
    "id": "property-parent-stage34-nested-no-default",
    "source": "class C { child: Int }\nfunction make(): C = new { child = new {} }\nresult = make().child.toString()\n",
    "sourceSha256": "826d464aae3553440b56deb46dac84287e37ff7e0bdaad234d75a4a5c4f6b074",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-no-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4bb42abfa3411ae1090b3f5cdb7143bd518533c67c77e171b695e5f4c104e219",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n2 | function make(): C = new { child = new {} }\n                                       ^^^\nat m2-platform#make.child (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n3 | result = make().child.toString()\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "property-parent-stage34-nested-union-no-default",
    "source": "class Child { x: Int = 7 }\nclass C { child: Child | String }\nfunction make(): C = new { child = new {} }\nresult = make().child.toString()\n",
    "sourceSha256": "4e7605e23ff9a0a7328c250b6b1fe71f0ae75e6c39efd398eb1330e1750feba5",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-nested-union-no-default",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e0c4f8ff92e5040faabfd36071c348b37c3ef67eefb0901f7bcb87b73fd6765e",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nCannot tell which parent to amend.\n\n3 | function make(): C = new { child = new {} }\n                                       ^^^\nat m2-platform#make.child (file:///m2-platform.pkl)\n\nPlease specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.\n\n4 | result = make().child.toString()\n             ^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "property-parent-stage34-prototype-inherited-default",
    "source": "class C { x: Int = 7 }\nopen class A { p: C }\nclass B extends A { p = new {} }\nresult = new B {}.p.x.toString()\n",
    "sourceSha256": "eb94040a9eea34373968843c6071bea9198234f73ca24dfa4c9f23af352843cf",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-prototype-inherited-default",
    "captureSha256": {
      "stdout": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "7"
  },
  {
    "id": "property-parent-stage34-prototype-inherited-initializer",
    "source": "class C { x: Int = 7 }\nopen class A { p: C = new { x = 9 } }\nclass B extends A { p = new {} }\nresult = new B {}.p.x.toString()\n",
    "sourceSha256": "f8fe10aa4a630f8e4f8edf4ddd8846351b03149ae95c806b4cfed53212c6e4db",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-prototype-inherited-initializer",
    "captureSha256": {
      "stdout": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "7"
  },
  {
    "id": "property-parent-stage34-shorthand-local-method",
    "source": "class C { x: Int = 7 }\none { two { local function make(): C = new {}; value = make().x } }\nresult = one.two.value.toString()\n",
    "sourceSha256": "cee24c9674dcab069bfe18a8bc127abf38f43370059e6679aa83b087c5a2c096",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-shorthand-local-method",
    "captureSha256": {
      "stdout": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "7"
  },
  {
    "id": "property-parent-stage34-shorthand-nested",
    "source": "one { two { value = 3 } }\nresult = (one.two is Dynamic).toString() + \":\" + one.two.value.toString()\n",
    "sourceSha256": "18a64451f48638934fcf3221a893497822c8c37d9abc5649bbd4518d21e08837",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-shorthand-nested",
    "captureSha256": {
      "stdout": "574dc7ee3adc9988f17ac1e4e0eb386006bf9a11b63c10c559323b9ffbb3e9a0",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "true:3"
  },
  {
    "id": "property-parent-stage34-origin-inherited-nested-listing-any",
    "source": "open class A { items: Listing<Listing<Any>> }\nclass B extends A { items = new { new { new {} } } }\nresult = (new B {}).items[0][0].toString()\n",
    "sourceSha256": "ebef5bebef161c0473b35b54773792a4d274d52d1b6928082002ca8c90576ad0",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-origin-inherited-nested-listing-any",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "c687e7a540e99dbd2a625bb824c229cda786cf5175890faa7f68a2f0901e056e",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `default` but its value is undefined.\n\n1 | open class A { items: Listing<Listing<Any>> }\n                          ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#A.items.default (file:///m2-platform.pkl)\n\n2 | class B extends A { items = new { new { new {} } } }\n                                            ^^^\nat m2-platform#B.items[#1][#1] (file:///m2-platform.pkl)\n\n3 | result = (new B {}).items[0][0].toString()\n             ^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "property-parent-stage34-origin-listing-any",
    "source": "p: Listing<Any> = new { new {} }\nresult = p[0].toString()\n",
    "sourceSha256": "9ab991f99f89b3c33530ae7d7c5727de6c14c6f3b2679f3b1f905977bfd8e9e2",
    "expression": "result",
    "oracleCasePath": "target/m2/stage34-property-selected-oracle-01/cases/property-parent-stage34-origin-listing-any",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "62cf2b0f323232003c40cc8c3a257010ce548f3f2ac8960d2fc0af6c22e39e33",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nTried to read property `default` but its value is undefined.\n\n1 | p: Listing<Any> = new { new {} }\n       ^^^^^^^^^^^^\nat m2-platform#p.default (file:///m2-platform.pkl)\n\n1 | p: Listing<Any> = new { new {} }\n                            ^^^\nat m2-platform#p[#1] (file:///m2-platform.pkl)\n\n2 | result = p[0].toString()\n             ^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  }
];

// Common fixture interface for the portable language comparison.
export { methodParentCases as cases, methodParentContext as context };

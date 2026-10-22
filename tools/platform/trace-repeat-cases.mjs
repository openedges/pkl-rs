// Exact fresh pinned Java scalar values/errors; includes two preserved Java internal errors.
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
        "reportPath": "target/m2/stage37-repeat-selected-oracle-01/report.json",
        "reportSha256": "b0153c626e6484f6b5470b0cbef7faaae3425324bb49aca28ea5a3364d4ad7d7",
        "payloadPath": "target/m2/stage37-repeat-selected-oracle-01/fixed-payloads.json",
        "payloadSha256": "0e1537fd880f8fbe77675d652b8cbb030b88132978eb19466b0b3201be0447ae",
        "cases": 20
      }
    ]
  }
};

export const cases = [
  {
    "id": "trace-stage37-repeat-zero",
    "source": "result=\"ab\".repeat(0)\n",
    "sourceSha256": "f770c43111a2cddc49d1eddd00a44810c3ef692d4c1399ede4a6b13c43fb34d5",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-zero",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": ""
  },
  {
    "id": "trace-stage37-repeat-one",
    "source": "result=\"ab\".repeat(1)\n",
    "sourceSha256": "c8850b9347fe5a97f204d93efd93935ec2ebf53a8a90c5eae014ed4be34f751d",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-one",
    "captureSha256": {
      "stdout": "fb8e20fc2e4c3f248c60c39bd652f3c1347298bb977b8b4d5903b85055620603",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ab"
  },
  {
    "id": "trace-stage37-repeat-normal",
    "source": "result=\"ab\".repeat(3)\n",
    "sourceSha256": "a375ec0c36ddab4614f6050d3b15e44270848007dfc693eb8c427a2fa20d3819",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-normal",
    "captureSha256": {
      "stdout": "36ff120f98d1ca85de299f65314b3b968d132cee69f8f8bb6a6d4e1058313355",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "ababab"
  },
  {
    "id": "trace-stage37-repeat-negative",
    "source": "result=\"ab\".repeat(-1)\n",
    "sourceSha256": "88f09aa5f8da151dd293c0eb4f8abc8fc5098bfea8890d48f4d81954c705a68c",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-negative",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "aa3c161f331a40394832de629d2c78dd6623730b8f852c79845c11dc0c765834",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nType constraint `isPositive` violated.\nValue: -1\n\n    isPositive\n    \u2502\n    false\n\n1122 | typealias UInt = Int(isPositive)\n                            ^^^^^^^^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1122)\n\n1 | result=\"ab\".repeat(-1)\n           ^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-float",
    "source": "result=\"ab\".repeat(1.0)\n",
    "sourceSha256": "78b9d98c04731ff1c27113e10a6f25bec4b04983bd6ad7d83457e54a88b96dec",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-float",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "77d7f1f626fd3834cb89d68bfca2b2fc83165386e53f93518832886c76c3d830",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `Float`.\nValue: 1.0\n\n1122 | typealias UInt = Int(isPositive)\n                        ^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1122)\n\n1 | result=\"ab\".repeat(1.0)\n           ^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-string",
    "source": "result=\"ab\".repeat(\"2\")\n",
    "sourceSha256": "c004fa4cfe734146cdf4171147a57dee605fc830a095661db287c626d4cb0b13",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-string",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "3e9b790ce409f0986089ca4d5ea34c6c29d681a256ff2af79d54be066751fdf1",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected value of type `Int`, but got type `String`.\nValue: \"2\"\n\n1122 | typealias UInt = Int(isPositive)\n                        ^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1122)\n\n1 | result=\"ab\".repeat(\"2\")\n           ^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-arity-zero",
    "source": "result=\"ab\".repeat()\n",
    "sourceSha256": "a5f5a3b43aa63fa74eabf3ab7384a313ef1126efa814a3d32694ea63a60f4e42",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-arity-zero",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "55fbf5e940c482c42421923f42b1f1afc1e932e798573d03b3e3fd73a21bdf89",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected 1 function arguments but got 0.\n\n1454 | external function repeat(count: UInt): String\n       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1454)\n\n1 | result=\"ab\".repeat()\n           ^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-arity-two-throw",
    "source": "result=\"ab\".repeat(-1,throw(\"extra\"))\n",
    "sourceSha256": "5db53d701ae605c569293e9362343516e973af11523024c39790a08aca4c8151",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-arity-two-throw",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d887bb4bed330f2f92100841910605a5e9e533006c7c5c332c419f210022084f",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nextra\n\n1 | result=\"ab\".repeat(-1,throw(\"extra\"))\n                          ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-argument-throw",
    "source": "result=\"ab\".repeat(throw(\"count\"))\n",
    "sourceSha256": "ebadf10fccc71bdff9d9b98f081d9572fed7059218072ae16ea77162e5f73835",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-argument-throw",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "d2574aadce5ef1096a3ebef44792f38716138bcf7c1f279819460e05cc7c1b79",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\ncount\n\n1 | result=\"ab\".repeat(throw(\"count\"))\n                       ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-receiver-throw",
    "source": "result=(throw(\"receiver\")).repeat(-1)\n",
    "sourceSha256": "845f1c767002fb83cc557c177b1e9924964c5da3ead037fe6388de95babe8a34",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-receiver-throw",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "2784744eb460459dad0651145c56d916b6c809b7181a878d827ec7525b95e139",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nreceiver\n\n1 | result=(throw(\"receiver\")).repeat(-1)\n            ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-empty-zero",
    "source": "result=\"\".repeat(0)\n",
    "sourceSha256": "557a93314bda468656add701dd9dedd9dba94de878d05516abf863696780012f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-empty-zero",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": ""
  },
  {
    "id": "trace-stage37-repeat-empty-cast-negative",
    "source": "result=\"\".repeat(2147483648)\n",
    "sourceSha256": "df113ca30a8979e4ef9e291ffce31b5458120cf63024fd85c6ad647937561615",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-empty-cast-negative",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "72dc825df227d617fecbebc17bdad93f1a1588656decfbfbdd55f5984502103c",
      "outcome": "937cc2827528278989a4008d6a26c92d7f4b616cc3e8faf442acc5b7f1ce1517"
    },
    "expectedError": "An unexpected error has occurred. Would you mind filing a bug report?\nCmd+Double-click the link below to open an issue.\nPlease copy and paste the entire error output into the issue's description.\n\nhttps://github.com/apple/pkl/issues/new\n\njava.lang.IllegalArgumentException%3A%20count%20is%20negative%3A%20-2147483648\n\n\u2013\u2013 Pkl Error \u2013\u2013\ncount is negative: -2147483648\n\n1454 | external function repeat(count: UInt): String\n       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1454)\n\n1 | result=\"\".repeat(2147483648)\n           ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n\nPkl 0.32.0 (macOS 27.0, Java 21.0.12.1)\n\njava.lang.IllegalArgumentException: count is negative: -2147483648\n\tat java.base/java.lang.String.repeat(String.java:4651)\n\tat org.pkl.core.stdlib.base.StringNodes$repeat.eval(StringNodes.java:284)\n\tat org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen.executeAndSpecialize(StringNodesFactory.java:1305)\n\tat org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen.executeGeneric_generic1(StringNodesFactory.java:1294)\n\tat org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen.executeGeneric(StringNodesFactory.java:1252)\n\tat org.pkl.core.ast.member.FunctionNode.executeImpl(FunctionNode.java:116)\n\tat org.pkl.core.ast.PklRootNode.execute(PklRootNode.java:46)\n\tat com.oracle.truffle.api.impl.DefaultCallTarget.call(DefaultCallTarget.java:118)\n\tat com.oracle.truffle.api.impl.DefaultDirectCallNode.call(DefaultDirectCallNode.java:59)\n\tat org.pkl.core.ast.expression.member.InvokeMethodVirtualNode.evalCached(InvokeMethodVirtualNode.java:137)\n\tat org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen.executeAndSpecialize(InvokeMethodVirtualNodeGen.java:268)\n\tat org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen.executeGeneric(InvokeMethodVirtualNodeGen.java:189)\n\tat org.pkl.core.ast.member.TypeCheckedPropertyNode.evalTypedObjectCached(TypeCheckedPropertyNode.java:54)\n\tat org.pkl.core.ast.member.TypeCheckedPropertyNodeGen.executeAndSpecialize(TypeCheckedPropertyNodeGen.java:132)\n\tat org.pkl.core.ast.member.TypeCheckedPropertyNodeGen.executeImpl(TypeCheckedPropertyNodeGen.java:97)\n\tat org.pkl.core.ast.PklRootNode.execute(PklRootNode.java:46)\n\tat com.oracle.truffle.api.impl.DefaultCallTarget.call(DefaultCallTarget.java:118)\n\tat com.oracle.truffle.api.impl.DefaultIndirectCallNode.call(DefaultIndirectCallNode.java:53)\n\tat org.pkl.core.runtime.VmUtils.doReadMember(VmUtils.java:395)\n\tat org.pkl.core.runtime.VmUtils.readMemberOrNull(VmUtils.java:324)\n\tat org.pkl.core.ast.expression.member.ReadPropertyNode.evalObject(ReadPropertyNode.java:112)\n\tat org.pkl.core.ast.expression.member.ReadPropertyNodeGen.executeAndSpecialize(ReadPropertyNodeGen.java:142)\n\tat org.pkl.core.ast.expression.member.ReadPropertyNodeGen.executeGeneric(ReadPropertyNodeGen.java:104)\n\tat org.pkl.core.ast.SimpleRootNode.executeImpl(SimpleRootNode.java:53)\n\tat org.pkl.core.ast.PklRootNode.execute(PklRootNode.java:46)\n\tat com.oracle.truffle.api.impl.DefaultCallTarget.call(DefaultCallTarget.java:118)\n\tat com.oracle.truffle.api.impl.DefaultIndirectCallNode.call(DefaultIndirectCallNode.java:53)\n\tat org.pkl.core.runtime.VmUtils.evaluateExpression(VmUtils.java:969)\n\tat org.pkl.core.EvaluatorImpl.lambda$evaluateExpressionString$0(EvaluatorImpl.java:267)\n\tat org.pkl.core.EvaluatorImpl.lambda$doEvaluate$0(EvaluatorImpl.java:470)\n\tat org.pkl.core.EvaluatorImpl.doEvaluate(EvaluatorImpl.java:388)\n\tat org.pkl.core.EvaluatorImpl.doEvaluate(EvaluatorImpl.java:466)\n\tat org.pkl.core.EvaluatorImpl.evaluateExpressionString(EvaluatorImpl.java:263)\n\tat Stage37RepeatFixedProbe.main(Stage37RepeatFixedProbe.java:24)\n"
  },
  {
    "id": "trace-stage37-repeat-empty-cast-zero",
    "source": "result=\"\".repeat(4294967296)\n",
    "sourceSha256": "d77253ecdb43514b233a147edab1260a446964693dc263a27cad1b7f5310a5ee",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-empty-cast-zero",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": ""
  },
  {
    "id": "trace-stage37-repeat-empty-max-count",
    "source": "result=\"\".repeat(9223372036854775807)\n",
    "sourceSha256": "92b60dae7e8ea1d8003dfea63a4571a28c2688c328a99079f972b5f2a8822aa9",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-empty-max-count",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "0be81bb235df1647c05eb498a6e0e0081815feb3aecb239855f792dbfd638e34",
      "outcome": "937cc2827528278989a4008d6a26c92d7f4b616cc3e8faf442acc5b7f1ce1517"
    },
    "expectedError": "An unexpected error has occurred. Would you mind filing a bug report?\nCmd+Double-click the link below to open an issue.\nPlease copy and paste the entire error output into the issue's description.\n\nhttps://github.com/apple/pkl/issues/new\n\njava.lang.IllegalArgumentException%3A%20count%20is%20negative%3A%20-1\n\n\u2013\u2013 Pkl Error \u2013\u2013\ncount is negative: -1\n\n1454 | external function repeat(count: UInt): String\n       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1454)\n\n1 | result=\"\".repeat(9223372036854775807)\n           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n\nPkl 0.32.0 (macOS 27.0, Java 21.0.12.1)\n\njava.lang.IllegalArgumentException: count is negative: -1\n\tat java.base/java.lang.String.repeat(String.java:4651)\n\tat org.pkl.core.stdlib.base.StringNodes$repeat.eval(StringNodes.java:284)\n\tat org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen.executeAndSpecialize(StringNodesFactory.java:1305)\n\tat org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen.executeGeneric_generic1(StringNodesFactory.java:1294)\n\tat org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen.executeGeneric(StringNodesFactory.java:1252)\n\tat org.pkl.core.ast.member.FunctionNode.executeImpl(FunctionNode.java:116)\n\tat org.pkl.core.ast.PklRootNode.execute(PklRootNode.java:46)\n\tat com.oracle.truffle.api.impl.DefaultCallTarget.call(DefaultCallTarget.java:118)\n\tat com.oracle.truffle.api.impl.DefaultDirectCallNode.call(DefaultDirectCallNode.java:59)\n\tat org.pkl.core.ast.expression.member.InvokeMethodVirtualNode.evalCached(InvokeMethodVirtualNode.java:137)\n\tat org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen.executeAndSpecialize(InvokeMethodVirtualNodeGen.java:268)\n\tat org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen.executeGeneric(InvokeMethodVirtualNodeGen.java:189)\n\tat org.pkl.core.ast.member.TypeCheckedPropertyNode.evalTypedObjectCached(TypeCheckedPropertyNode.java:54)\n\tat org.pkl.core.ast.member.TypeCheckedPropertyNodeGen.executeAndSpecialize(TypeCheckedPropertyNodeGen.java:132)\n\tat org.pkl.core.ast.member.TypeCheckedPropertyNodeGen.executeImpl(TypeCheckedPropertyNodeGen.java:97)\n\tat org.pkl.core.ast.PklRootNode.execute(PklRootNode.java:46)\n\tat com.oracle.truffle.api.impl.DefaultCallTarget.call(DefaultCallTarget.java:118)\n\tat com.oracle.truffle.api.impl.DefaultIndirectCallNode.call(DefaultIndirectCallNode.java:53)\n\tat org.pkl.core.runtime.VmUtils.doReadMember(VmUtils.java:395)\n\tat org.pkl.core.runtime.VmUtils.readMemberOrNull(VmUtils.java:324)\n\tat org.pkl.core.ast.expression.member.ReadPropertyNode.evalObject(ReadPropertyNode.java:112)\n\tat org.pkl.core.ast.expression.member.ReadPropertyNodeGen.executeAndSpecialize(ReadPropertyNodeGen.java:142)\n\tat org.pkl.core.ast.expression.member.ReadPropertyNodeGen.executeGeneric(ReadPropertyNodeGen.java:104)\n\tat org.pkl.core.ast.SimpleRootNode.executeImpl(SimpleRootNode.java:53)\n\tat org.pkl.core.ast.PklRootNode.execute(PklRootNode.java:46)\n\tat com.oracle.truffle.api.impl.DefaultCallTarget.call(DefaultCallTarget.java:118)\n\tat com.oracle.truffle.api.impl.DefaultIndirectCallNode.call(DefaultIndirectCallNode.java:53)\n\tat org.pkl.core.runtime.VmUtils.evaluateExpression(VmUtils.java:969)\n\tat org.pkl.core.EvaluatorImpl.lambda$evaluateExpressionString$0(EvaluatorImpl.java:267)\n\tat org.pkl.core.EvaluatorImpl.lambda$doEvaluate$0(EvaluatorImpl.java:470)\n\tat org.pkl.core.EvaluatorImpl.doEvaluate(EvaluatorImpl.java:388)\n\tat org.pkl.core.EvaluatorImpl.doEvaluate(EvaluatorImpl.java:466)\n\tat org.pkl.core.EvaluatorImpl.evaluateExpressionString(EvaluatorImpl.java:263)\n\tat Stage37RepeatFixedProbe.main(Stage37RepeatFixedProbe.java:24)\n"
  },
  {
    "id": "trace-stage37-repeat-overflow-int32",
    "source": "result=\"ab\".repeat(1073741824)\n",
    "sourceSha256": "1a3b91981011222f1b10797729aa0a4377eb80602dab5dd991041d5730e57d45",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-overflow-int32",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "6cdc11ac87ea5fc3635df209b829f789f4fb40a5c468cb35eeb7c5036054edd4",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInt value `2,147,483,648` is too large (only Int32 supported here).\n\n1 | result=\"ab\".repeat(1073741824)\n           ^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-overflow-int64",
    "source": "result=\"ab\".repeat(9223372036854775807)\n",
    "sourceSha256": "2bd56fbaa1b7edaeb280df82eb7daa2f6667ad38ec06ccb670f7e4a25b9adc83",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-overflow-int64",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "77ff041e39b3308c66abc9ecc5c1f65b6c1c6f625e7392480e9d0859c3bd1026",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInteger overflow.\n\n1 | result=\"ab\".repeat(9223372036854775807)\n           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-supplementary-overflow",
    "source": "result=\"\ud83d\ude00\".repeat(1073741824)\n",
    "sourceSha256": "d1cba8b616be5aa316918490426cd497997d056283424300907a841f975696d6",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-supplementary-overflow",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4ded966226d7529c9f6e5fbdb1e0bd11a2614c19bf3f80d5e569468daa1c4755",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nInt value `2,147,483,648` is too large (only Int32 supported here).\n\n1 | result=\"\ud83d\ude00\".repeat(1073741824)\n           ^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  },
  {
    "id": "trace-stage37-repeat-lone-surrogate",
    "source": "result=\"\\u{D800}\".repeat(3)\n",
    "sourceSha256": "5dd8eeff00cf163dc95084b10d1ab50ccf17ad3fb3c7f828d4fe0e22a2e97f42",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-lone-surrogate",
    "captureSha256": {
      "stdout": "a03b221c6c6eae7122ca51695d456d5222e524889136394944b2f9763b483615",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "???"
  },
  {
    "id": "trace-stage37-repeat-supplementary-normal",
    "source": "result=\"\ud83d\ude00\".repeat(2)\n",
    "sourceSha256": "4a8fa57d7b196f36a5eff568cd419f8f4678244b11dbb00f973c6ceae072946e",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-supplementary-normal",
    "captureSha256": {
      "stdout": "91ad485bf369a67f134962f4726eac8fc299a51f1504fc6835210fb32c4890f1",
      "stderr": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "outcome": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5"
    },
    "expectedValue": "\ud83d\ude00\ud83d\ude00"
  },
  {
    "id": "trace-stage37-repeat-arity-two-before-constraint",
    "source": "result=\"ab\".repeat(-1,2)\n",
    "sourceSha256": "698e9140c267f615e718139485d0f0d57151741092a7c9b25a72f2863f98d51f",
    "expression": "result",
    "oracleCasePath": "target/m2/stage37-repeat-selected-oracle-01/cases/trace-stage37-repeat-arity-two-before-constraint",
    "captureSha256": {
      "stdout": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "stderr": "4ebf3755a9a79f434798394e8b7750aa0572e41b654649462d809085a6f9e122",
      "outcome": "36d78792e56327594711020ed2848fa989834ce17b2cc5d2702f914d023842cf"
    },
    "expectedError": "\u2013\u2013 Pkl Error \u2013\u2013\nExpected 1 function arguments but got 2.\n\n1454 | external function repeat(count: UInt): String\n       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.base#String.repeat (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1454)\n\n1 | result=\"ab\".repeat(-1,2)\n           ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
  }
];

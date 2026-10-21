// Complete raw pinned Java 0.32.0 captures; expected results never come from Rust.
// Source and runtime identities refer to the retained acquisition below.
export const typeMetadataOracle = {
  "acquisition": "target/m2/stage13-sequences-java-explore-01",
  "oracleMetadataSha256": "18f25c1cdcdb4e47cb894adc172e7af1e526c9cf38b8212d0c151fe34f36a7a2",
  "oracleVersion": "0.32.0",
  "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
  "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
  "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
  "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
  "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d",
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false
};

export const typeMetadataCases = [
  {
    "id": "alias-values",
    "scenario": "m2-type-alias-values",
    "source": "typealias A = String\ntypealias B = String\nidentity = A == A\ndistinct = A != B\nkind = A.getClass() == TypeAlias\nname = A.toString()\nunion: A | Int = true\n",
    "expressions": [
      "A",
      "identity",
      "distinct",
      "kind",
      "name",
      "union",
      "A"
    ],
    "expected": [
      "value\tm2-platform#A",
      "value\ttrue",
      "value\ttrue",
      "value\ttrue",
      "value\tm2-platform#A",
      "error\t–– Pkl Error ––\nExpected value of type `A | Int`, but got a different `Boolean`.\nValue: true\n\n7 | union: A | Int = true\n           ^^^^^^^\nat m2-platform#union (file:///m2-platform.pkl)\n\n* Value is not of type `A` because:\n  Expected value of type `String`, but got type `Boolean`.\n  Value: true\n\n7 | union: A | Int = true\n                     ^^^^\nat m2-platform#union (file:///m2-platform.pkl)\n\n1 | union\n    ^^^^^\nat  (repl:text)\n",
      "value\tm2-platform#A"
    ],
    "sourceSha256": "73b65d4eb2be8330f280b26bf76ff93ca672aeb787c5f78e21d94c68f8635eb6",
    "expressionsSha256": "a7f7ebc8e353380825b3e15e53859353fa8933ff06e76560a4e102bdf33217b6",
    "captureSha256": {
      "0.stdout.bin": "157dcd78e977f472f1d1ded18c0c61461d32df9f6e5ff3c17690a9e0e17c7ffb",
      "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "2.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "3.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "4.stdout.bin": "157dcd78e977f472f1d1ded18c0c61461d32df9f6e5ff3c17690a9e0e17c7ffb",
      "5.stderr.bin": "8e82de84fe39a4c0108b9d6a78a83e143360c089e3dbc5581c30d5d5871bf5e4",
      "6.stdout.bin": "157dcd78e977f472f1d1ded18c0c61461d32df9f6e5ff3c17690a9e0e17c7ffb"
    }
  },
  {
    "id": "function-metadata",
    "scenario": "m2-function-type-metadata",
    "source": "typealias Fn = (Int) -> String\nlocal f: Fn = (x) -> 3\nvalue = f.apply(1)\nlocal bad: Fn = () -> \"bad\"\narity = bad\nlocal wrong: Fn = 2\nwrongValue = wrong\nidentity = Fn == Fn\n",
    "expressions": [
      "Fn",
      "value",
      "arity",
      "wrongValue",
      "identity",
      "value"
    ],
    "expected": [
      "value\tm2-platform#Fn",
      "value\t3",
      "error\t–– Pkl Error ––\nExpected value of type `Function1`, but got type `Function0`.\nValue: new Function0 {}\n\n1 | typealias Fn = (Int) -> String\n                   ^^^^^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n4 | local bad: Fn = () -> \"bad\"\n                    ^^^^^^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n5 | arity = bad\n            ^^^\nat m2-platform#arity (file:///m2-platform.pkl)\n\n1 | arity\n    ^^^^^\nat  (repl:text)\n",
      "error\t–– Pkl Error ––\nExpected value of type `Function1`, but got type `Int`.\nValue: 2\n\n1 | typealias Fn = (Int) -> String\n                   ^^^^^^^^^^^^^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n6 | local wrong: Fn = 2\n                      ^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n7 | wrongValue = wrong\n                 ^^^^^\nat m2-platform#wrongValue (file:///m2-platform.pkl)\n\n1 | wrongValue\n    ^^^^^^^^^^\nat  (repl:text)\n",
      "value\ttrue",
      "value\t3"
    ],
    "sourceSha256": "f24e6afbee290a88f8fb6a65ef848dfd377647a529863177113d9bd7d903f3d6",
    "expressionsSha256": "b7263d2d433e696c4e00770257fb5ca172e75ebafaff48cd40c87971dff2af56",
    "captureSha256": {
      "0.stdout.bin": "2d0f6ae7d88b176adc88ed23cf55d3e0cffb40513ab4fb7fe90108e86c72d3ed",
      "1.stdout.bin": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce",
      "2.stderr.bin": "b4004b41c227c13e9e729d1e8aa263698ddb4957f16f51a990c49cd6f7069046",
      "3.stderr.bin": "eb1db12c3b1d242f27f46dec4a84cea1ab2483e43573d74e0a9171698d7fa244",
      "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "5.stdout.bin": "4e07408562bedb8b60ce05c1decfe3ad16b72230967de01f640b7e4729b49fce"
    }
  },
  {
    "id": "alias-generic",
    "scenario": "m2-generic-alias-diagnostics",
    "source": "typealias Identity<T> = T\ntypealias Text = String\ngenericIdentity = Identity == Identity\nseparate = Identity != Text\nkind = Identity.getClass() == TypeAlias\nuntyped: Identity = 7\nvalid: Identity<Text> = \"ok\"\nbad: Identity<Text> = 2\n",
    "expressions": [
      "Identity",
      "genericIdentity",
      "separate",
      "kind",
      "untyped",
      "valid",
      "bad",
      "valid",
      "Identity"
    ],
    "expected": [
      "value\tm2-platform#Identity",
      "value\ttrue",
      "value\ttrue",
      "value\ttrue",
      "value\t7",
      "value\tok",
      "error\t–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 2\n\n2 | typealias Text = String\n                     ^^^^^^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n8 | bad: Identity<Text> = 2\n                          ^\nat m2-platform#bad (file:///m2-platform.pkl)\n\n1 | bad\n    ^^^\nat  (repl:text)\n",
      "value\tok",
      "value\tm2-platform#Identity"
    ],
    "sourceSha256": "3b9b75a5e74661d75094af7eb18dbd375944cec3726115cc964ebb853ce4056a",
    "expressionsSha256": "a281e71298b7288c47b41022739aa6c9a85bb13148d3479206c7ee7215c7d60b",
    "captureSha256": {
      "0.stdout.bin": "70fb30d02bb9ca697fea17f79a2bf38b270bb9c5845620bdb91990a9e193a883",
      "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "2.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "3.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
      "4.stdout.bin": "7902699be42c8a8e46fbbb4501726517e86b22c56a189f7625a6da49081b2451",
      "5.stdout.bin": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "6.stderr.bin": "de9e77d207cbec65a775bc859ed8c2392e88dfcf0a3aa803b581fc6c0ea3e0a4",
      "7.stdout.bin": "2689367b205c16ce32ed4200942b8b8b1e262dfc70d9bc9fbc77c49699a4f1df",
      "8.stdout.bin": "70fb30d02bb9ca697fea17f79a2bf38b270bb9c5845620bdb91990a9e193a883"
    }
  }
];

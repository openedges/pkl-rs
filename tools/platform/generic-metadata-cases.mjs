// Complete pinned Java 0.32.0 captures at the identical module URI.
// Shared metadata stress has separate native stack and lifecycle assertions.
export const genericMetadataContext = {
  "uri": "file:///m2-platform.pkl",
  "moduleName": "m2-platform",
  "powerAssertions": false,
  "oracle": {
    "acquisition": "target/m2/stage19-sequence-baseline-01",
    "oracleMetadataSha256": "18f25c1cdcdb4e47cb894adc172e7af1e526c9cf38b8212d0c151fe34f36a7a2",
    "oracleVersion": "0.32.0",
    "oracleJarSha256": "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
    "oracleLockSha256": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "javaExecutableSha256": "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "javaRuntimeModulesSha256": "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "referenceRevision": "e51fb2f91774265666f94980ec630db794a4db2d"
  }
};

export const genericMetadataGroups = [
  {
    "scenario": "m2-generic-metadata-deep-sequences",
    "fixtures": [
      {
        "id": "deep-actuals",
        "source": "typealias A<T> = Int(this < 0 || (this - 1) is A<List<T>>)\none: A<Int> = 256\ntwo: A<String> = 1024\nwrong: A<Int> = \"bad\"\nstable = 42\n",
        "expressions": [
          "one",
          "wrong",
          "two",
          "wrong",
          "one",
          "stable",
          "two"
        ],
        "expected": [
          "value\t256",
          "error\t–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"bad\"\n\n1 | typealias A<T> = Int(this < 0 || (this - 1) is A<List<T>>)\n                     ^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n4 | wrong: A<Int> = \"bad\"\n                    ^^^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n1 | wrong\n    ^^^^^\nat  (repl:text)\n",
          "value\t1024",
          "error\t–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"bad\"\n\n1 | typealias A<T> = Int(this < 0 || (this - 1) is A<List<T>>)\n                     ^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n4 | wrong: A<Int> = \"bad\"\n                    ^^^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n1 | wrong\n    ^^^^^\nat  (repl:text)\n",
          "value\t256",
          "value\t42",
          "value\t1024"
        ],
        "sourceSha256": "609f9d1c37ef24b5bed304bc706a8f73e7f4fc678219fd738c43df2670e443c7",
        "expressionsSha256": "d92706d69cd9d3b7e6b25530e4c3ba7f9d14e92cd76f806ca6b7f85c8bf208a4",
        "captureSha256": {
          "0.stdout.bin": "51e8ea280b44e16934d4d611901f3d3afc41789840acdff81942c2f65009cd52",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stderr.bin": "4364c04875dcfb6505c1578911995770ecec9507976c8aa31f037357c4b6452b",
          "1.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "1.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "2.stdout.bin": "e39eef82f61b21e2e7f762fcc4307358f165757f2e77ec855d6992f7e0191932",
          "2.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "2.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stderr.bin": "4364c04875dcfb6505c1578911995770ecec9507976c8aa31f037357c4b6452b",
          "3.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "3.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "4.stdout.bin": "51e8ea280b44e16934d4d611901f3d3afc41789840acdff81942c2f65009cd52",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "5.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "5.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "6.stdout.bin": "e39eef82f61b21e2e7f762fcc4307358f165757f2e77ec855d6992f7e0191932",
          "6.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "6.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "6.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      },
      {
        "id": "branch-actuals",
        "source": "typealias A<T> = Int(this < 0 || (this - 1) is A<Mapping<T,T>>)\none: A<Int> = 14\ntwo: A<String> = 18\nwrong: A<Int> = \"bad\"\nstable = 42\n",
        "expressions": [
          "one",
          "two",
          "wrong",
          "two",
          "one",
          "stable"
        ],
        "expected": [
          "value\t14",
          "value\t18",
          "error\t–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"bad\"\n\n1 | typealias A<T> = Int(this < 0 || (this - 1) is A<Mapping<T,T>>)\n                     ^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n4 | wrong: A<Int> = \"bad\"\n                    ^^^^^\nat m2-platform#wrong (file:///m2-platform.pkl)\n\n1 | wrong\n    ^^^^^\nat  (repl:text)\n",
          "value\t18",
          "value\t14",
          "value\t42"
        ],
        "sourceSha256": "c3bd1ca270dda6e7e0127f6955647163427708cb68d859fb85d20bdffbc3d7bf",
        "expressionsSha256": "88c0b0373019d78d56b6e84a9dcabc634ea2d485b71c3347af93dee88b480063",
        "captureSha256": {
          "0.stdout.bin": "8527a891e224136950ff32ca212b45bc93f69fbb801c3b1ebedac52775f99e61",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "4ec9599fc203d176a301536c2e091a19bc852759b255bd6818810a42c5fed14a",
          "1.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "1.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.stderr.bin": "ee5cd3952ea26abd5385d0b6f7568ffe2056df0b9a221d335a8954b22b2372f3",
          "2.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "2.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "3.stdout.bin": "4ec9599fc203d176a301536c2e091a19bc852759b255bd6818810a42c5fed14a",
          "3.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "3.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.stdout.bin": "8527a891e224136950ff32ca212b45bc93f69fbb801c3b1ebedac52775f99e61",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "5.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "5.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      }
    ]
  },
  {
    "scenario": "m2-generic-metadata-default-sequences",
    "fixtures": [
      {
        "id": "swap-default-inline",
        "source": "class C {p = 1}\nclass D {p = 2}\ntypealias A<T,U> = Int(if (this < 0) (new Listing<T> {new {}})[0].p == 1 else (this - 1) is A<U,T>)\none = 32 is A<C,D>\ntwo = 32 is A<D,C>\nthree = 31 is A<C,D>\nstable = 42\n",
        "expressions": [
          "one",
          "two",
          "three",
          "one",
          "two",
          "stable"
        ],
        "expected": [
          "value\tfalse",
          "value\ttrue",
          "value\ttrue",
          "value\tfalse",
          "value\ttrue",
          "value\t42"
        ],
        "sourceSha256": "d8f4fbf50bb2f84f50ab9da87e97fc591dad8454be7dab78aff89d1ff7edce2b",
        "expressionsSha256": "59a0ecbb1c2450de038257f87c4097c83b0373063c5d96bf36f0554507e24fcb",
        "captureSha256": {
          "0.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "1.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "1.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "2.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "2.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "3.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "3.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "5.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "5.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      },
      {
        "id": "swap-default-lambda",
        "source": "class C {p = 1}\nclass D {p = 2}\ntypealias A<T,U> = Int((n) -> if (n < 0) (new Listing<T> {new {}})[0].p == 1 else (n - 1) is A<U,T>)\none = 32 is A<C,D>\ntwo = 32 is A<D,C>\nthree = 31 is A<C,D>\nstable = 42\n",
        "expressions": [
          "one",
          "two",
          "three",
          "one",
          "two",
          "stable"
        ],
        "expected": [
          "value\tfalse",
          "value\ttrue",
          "value\ttrue",
          "value\tfalse",
          "value\ttrue",
          "value\t42"
        ],
        "sourceSha256": "e9640ea4fa5e42318b364af31c3547bb17e30d6f384885d7a8bdca5665ba1b46",
        "expressionsSha256": "59a0ecbb1c2450de038257f87c4097c83b0373063c5d96bf36f0554507e24fcb",
        "captureSha256": {
          "0.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "1.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "1.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "2.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "2.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "3.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "3.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "5.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "5.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      },
      {
        "id": "alias-root-actuals",
        "source": "class C {p = 1}\nclass D {p = 2}\ntypealias Id<T> = T\ntypealias A<T> = Int((new Listing<Id<T>> {new {}})[0].p == 1)\none = 1 is A<C>\ntwo = 1 is A<D>\nthree = 1 is A<*C|String>\nstable = 42\n",
        "expressions": [
          "one",
          "two",
          "three",
          "two",
          "one",
          "stable"
        ],
        "expected": [
          "value\ttrue",
          "value\tfalse",
          "value\ttrue",
          "value\tfalse",
          "value\ttrue",
          "value\t42"
        ],
        "sourceSha256": "b97afa368db9b87bd906d41ebe394fa8baebee660d4f6698aea7c0c0b50e0944",
        "expressionsSha256": "df383ca53b90f1b069078be869d51b870b57a1da90f73773c4be8c9bd6db77b9",
        "captureSha256": {
          "0.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "1.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "1.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "2.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "2.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "3.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "3.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "5.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "5.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      }
    ]
  },
  {
    "scenario": "m2-generic-metadata-consumer-sequences",
    "fixtures": [
      {
        "id": "wrapper-actuals",
        "source": "typealias Check<T,U> = Int(((new Listing<T> {1}) as Listing<U>)[0] == 1)\none = 1 is Check<Int,Int>\ntwo = 1 is Check<Int,String>\nthree = 1 is Check<Int(this>0),Int(this<0)>\nstable = 42\n",
        "expressions": [
          "one",
          "two",
          "three",
          "two",
          "one",
          "three",
          "stable"
        ],
        "expected": [
          "value\ttrue",
          "error\t–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n3 | two = 1 is Check<Int,String>\n                         ^^^^^^\nat m2-platform#two (file:///m2-platform.pkl)\n\n1 | two\n    ^^^\nat  (repl:text)\n",
          "error\t–– Pkl Error ––\nType constraint `this<0` violated.\nValue: 1\n\n4 | three = 1 is Check<Int(this>0),Int(this<0)>\n                                       ^^^^^^\nat m2-platform#three (file:///m2-platform.pkl)\n\n1 | three\n    ^^^^^\nat  (repl:text)\n",
          "error\t–– Pkl Error ––\nExpected value of type `String`, but got type `Int`.\nValue: 1\n\n3 | two = 1 is Check<Int,String>\n                         ^^^^^^\nat m2-platform#two (file:///m2-platform.pkl)\n\n1 | two\n    ^^^\nat  (repl:text)\n",
          "value\ttrue",
          "error\t–– Pkl Error ––\nType constraint `this<0` violated.\nValue: 1\n\n4 | three = 1 is Check<Int(this>0),Int(this<0)>\n                                       ^^^^^^\nat m2-platform#three (file:///m2-platform.pkl)\n\n1 | three\n    ^^^^^\nat  (repl:text)\n",
          "value\t42"
        ],
        "sourceSha256": "23b4a0d0928e446966b50dd299fee5a6cafa1a3153324377cee61a2420287b18",
        "expressionsSha256": "83b656ffa854259ba941a07071c3b6534792ebd7bbe9cdf8cafc0901843748c0",
        "captureSha256": {
          "0.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stderr.bin": "be79d46d609f34b498f3aba25811aa2e4fd696135b6f527b0fcae6145720a5fd",
          "1.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "1.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "2.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.stderr.bin": "581b5be1c394ff74b58f7a44ff52b7181c49eb4e8a4e851ccb2e9c7f37b022f8",
          "2.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "2.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "3.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stderr.bin": "be79d46d609f34b498f3aba25811aa2e4fd696135b6f527b0fcae6145720a5fd",
          "3.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "3.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "4.stdout.bin": "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stderr.bin": "581b5be1c394ff74b58f7a44ff52b7181c49eb4e8a4e851ccb2e9c7f37b022f8",
          "5.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "5.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "6.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "6.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "6.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "6.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      },
      {
        "id": "reference-actuals",
        "source": "import \"pkl:ref\"\nclass D extends ref.Domain {}\ntypealias A<T> = Int((n) -> if (n < 0) 1 is ref.Reference<D,T> else (n - 1) is A<Mapping<T,T>>)\none = 6 is A<Int>\ntwo = 6 is A<Int(this > 0)>\nthree = -1 is A<Listing<Int>>\nstable = 42\n",
        "expressions": [
          "one",
          "two",
          "three",
          "two",
          "one",
          "stable"
        ],
        "expected": [
          "value\tfalse",
          "error\t–– Pkl Error ––\n`Reference` referent type argument may not include type constraints.\n\n3 | typealias A<T> = Int((n) -> if (n < 0) 1 is ref.Reference<D,T> else (n - 1) is A<Mapping<T,T>>)\n                                                ^^^^^^^^^^^^^^^^^^\nat m2-platform#A.<function#1> (file:///m2-platform.pkl)\n\n1 | two\n    ^^^\nat  (repl:text)\n",
          "value\tfalse",
          "error\t–– Pkl Error ––\n`Reference` referent type argument may not include type constraints.\n\n3 | typealias A<T> = Int((n) -> if (n < 0) 1 is ref.Reference<D,T> else (n - 1) is A<Mapping<T,T>>)\n                                                ^^^^^^^^^^^^^^^^^^\nat m2-platform#A.<function#1> (file:///m2-platform.pkl)\n\n1 | two\n    ^^^\nat  (repl:text)\n",
          "value\tfalse",
          "value\t42"
        ],
        "sourceSha256": "2258113636855a931cf90f148c4125006c8296ec4f7c9ce1bbaa344786fe632d",
        "expressionsSha256": "df383ca53b90f1b069078be869d51b870b57a1da90f73773c4be8c9bd6db77b9",
        "captureSha256": {
          "0.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "0.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "0.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "0.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "1.stderr.bin": "d2cee945229a603e49b005f8571ce6cb79d4022259df27f267269d03fee7a411",
          "1.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "1.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "2.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "2.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "2.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "2.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stdout.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "3.stderr.bin": "d2cee945229a603e49b005f8571ce6cb79d4022259df27f267269d03fee7a411",
          "3.status.bin": "f097b5f4f46cda2da21b954c9ff4097e1e14ae7064ecdee2c2cec2d3c1f08e6b",
          "3.error-class.bin": "4beef7d9db837350f4c17ee9d1ae0a6958cc46ba4afbad087ef08f57bdeae591",
          "4.stdout.bin": "fcbcf165908dd18a9e49f7ff27810176db8e9f63b4352213741664245224f8aa",
          "4.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "4.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "4.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.stdout.bin": "73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
          "5.stderr.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "5.status.bin": "81b2bd4ea98c8db66554fbc8d7637a1a69a130f331feb732b75caab4c4868fd5",
          "5.error-class.bin": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        }
      }
    ]
  }
];

import { FakeHostOperations, waitingEvaluation } from './host-adapter.mjs';
import { classPublicationCases } from './class-publication-cases.mjs';
import { typeMetadataCases, typeMetadataOracle } from './type-metadata-cases.mjs';
import { typeResolutionCases, typeResolutionOracle } from './type-resolution-cases.mjs';
import { recursiveTypesCases, recursiveTypesOracle } from './recursive-types-cases.mjs';
import { constraintSlotsCases, constraintSlotsOracle } from './constraint-slots-cases.mjs';
import { localResolutionGroups, localResolutionContext } from './local-resolution-cases.mjs';

// Identical assertions execute inside Node and an actual Chromium page.
export async function scenarios(api) {
  const passed = [];
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const eq = (actual, expected) => assert(actual === expected, `${String(actual)} != ${String(expected)}`);
  const throws = (fn, message) => {
    let caught;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof Error, 'expected a JavaScript Error');
    if (message) eq(caught.message, message);
  };
  const check = async (name, fn) => { await fn(); passed.push(name); };
  const clock = () => Math.floor(performance.now());
  const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms));
  const dispose = (probe) => { probe.close(); if (probe.free) probe.free(); };
  const read = probe => {
    const fields = probe.poll(clock(), 100).split('\t');
    eq(fields[0], 'read'); eq(fields[2], '746573743a2f7265736f75726365');
    return fields[1];
  };
  await check('shared-parser-resolver-vm', () => {
    eq(api.evaluate('answer = 40 + 2', 'answer'), 'int\t42');
    eq(api.evaluate('answer = "한글🙂"', 'answer'), 'string\ted959ceab880f09f9982');
    eq(api.evaluate('base = new { n = 1; value = this.n }; child = (base) { n = 3 }; answer = child.value', 'answer'), 'int\t3');
    throws(() => api.evaluate('answer = 1 +', 'answer'));
  });
  // These source cases also execute against Java in the M2 native differential
  // selection. Here they verify the same engine through real JS/Wasm bindings.
  await check('m2-recursive-methods-and-closures', () => {
    eq(api.evaluate('function sum(n) = if (n == 0) 0 else n + sum(n - 1)\nresult = sum(10)', 'result'), 'int\t55');
    eq(api.evaluate('result = let (x = 40) ((y) -> x + y).apply(2)', 'result'), 'int\t42');
    eq(api.evaluate('function add(x: Int, y: Int): Int = x + y\nresult = add(40, 2)', 'result'), 'int\t42');
  });
  await check('m2-lazy-control-and-interpolation', () => {
    eq(api.evaluate('result = 42 ?? missing', 'result'), 'int\t42');
    eq(api.evaluate('result = false && missing', 'result'), 'bool\tfalse');
    eq(api.evaluate('result = "answer: \\(40 + 2)"', 'result'), 'string\t616e737765723a203432');
  });
  await check('m2-shared-language-diagnostics', () => {
    const uri = 'file:///m2-platform.pkl';
    const source = 'result = 1 ~/ 0\n';
    throws(() => api.evaluateLanguage(source, uri, 'm2-platform', 'result'),
      '–– Pkl Error ––\nDivision by zero.\n\n1 | result = 1 ~/ 0\n             ^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n');
    eq(api.evaluateLanguage('result = "한글🙂"', uri, 'm2-platform', 'result'), '한글🙂');
  });
  await check('m2-class-publication-sequences', () => {
    for (const fixture of classPublicationCases) {
      // Run each list twice on fresh evaluators. Within a list, every failure
      // and retry shares one evaluator/module and the original call ordering.
      for (let repetition = 0; repetition < 2; repetition++) {
        const actual = api.evaluateLanguageSequence(fixture.source,
          'file:///m2-platform.pkl', 'm2-platform', fixture.expressions, false);
        eq(actual.length, fixture.expected.length);
        for (let index = 0; index < actual.length; index++) {
          eq(actual[index], fixture.expected[index]);
        }
      }
    }
  });
  for (const fixture of typeMetadataCases) {
    await check(fixture.scenario, () => {
      for (let repetition = 0; repetition < 2; repetition++) {
        const actual = api.evaluateLanguageSequence(fixture.source,
          typeMetadataOracle.uri, typeMetadataOracle.moduleName,
          fixture.expressions, typeMetadataOracle.powerAssertions);
        eq(actual.length, fixture.expected.length);
        for (let index = 0; index < actual.length; index++) {
          eq(actual[index], fixture.expected[index]);
        }
      }
    });
  }
  for (const fixture of typeResolutionCases) {
    await check(fixture.scenario, () => {
      for (let repetition = 0; repetition < 2; repetition++) {
        const actual = api.evaluateLanguageSequence(fixture.source,
          typeResolutionOracle.uri, typeResolutionOracle.moduleName,
          fixture.expressions, typeResolutionOracle.powerAssertions);
        eq(actual.length, fixture.expected.length);
        for (let index = 0; index < actual.length; index++) {
          eq(actual[index], fixture.expected[index]);
        }
      }
    });
  }
  for (const fixture of recursiveTypesCases) {
    await check(fixture.scenario, () => {
      for (let repetition = 0; repetition < 2; repetition++) {
        const actual = api.evaluateLanguageSequence(fixture.source,
          recursiveTypesOracle.uri, recursiveTypesOracle.moduleName,
          fixture.expressions, recursiveTypesOracle.powerAssertions);
        eq(actual.length, fixture.expected.length);
        for (let index = 0; index < actual.length; index++) {
          eq(actual[index], fixture.expected[index]);
        }
      }
    });
  }
  for (const fixture of constraintSlotsCases) {
    await check(fixture.scenario, () => {
      for (let repetition = 0; repetition < 2; repetition++) {
        const actual = api.evaluateLanguageSequence(fixture.source,
          constraintSlotsOracle.uri, constraintSlotsOracle.moduleName,
          fixture.expressions, constraintSlotsOracle.powerAssertions);
        eq(actual.length, fixture.expected.length);
        for (let index = 0; index < actual.length; index++) {
          eq(actual[index], fixture.expected[index]);
        }
      }
    });
  }
  for (const group of localResolutionGroups) {
    await check(group.scenario, () => {
      for (const fixture of group.fixtures) {
        for (let repetition = 0; repetition < 2; repetition++) {
          const actual = api.evaluateLanguageSequence(fixture.source,
            localResolutionContext.uri, localResolutionContext.moduleName,
            fixture.expressions, localResolutionContext.powerAssertions);
          eq(actual.length, fixture.expected.length);
          for (let index = 0; index < actual.length; index++) {
            eq(actual[index], fixture.expected[index]);
          }
        }
      }
    });
  }
  await check('m2-class-method-and-type-alias', () => {
    const source = 'typealias Count = Int\nclass Box { value: Count = 40; function add(n: Count): Count = value + n }\nbox = new Box {}\nresult = box.add(2)';
    eq(api.evaluateLanguage(source, 'file:///m2-platform.pkl', 'm2-platform', 'result'), '42');
    throws(() => api.evaluateLanguage('class C { x = 1 }\nc = new C { extra = 2 }\nresult = c.x\n',
      'file:///m2-platform.pkl', 'm2-platform', 'result'),
      '–– Pkl Error ––\nCannot find property `extra` in object of type `m2-platform#C`.\n\n2 | c = new C { extra = 2 }\n                ^^^^^\nat m2-platform#c (file:///m2-platform.pkl)\n\nAvailable properties:\nx\n\n3 | result = c.x\n             ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n');
  });
  await check('m2-pipeline-and-constraint-values', () => {
    const evaluate = source => api.evaluateLanguage(source, 'file:///m2-platform.pkl', 'm2-platform', 'result');
    eq(evaluate('result = 20 |> ((x) -> x + 1) |> ((x) -> x * 2)\n'), '42');
    eq(evaluate('result = let (n: Int(this > 0) = 42) n\n'), '42');
    eq(evaluate('result: Int(this < 0) | Int(this > 0) = 42\n'), '42');
    eq(evaluate('result = 42 is Int(this < 0)\n'), 'false');
    eq(evaluate('typealias Positive = Int((x) -> x > 0)\nresult: Positive = 42\n'), '42');
  });
  await check('m2-constraint-diagnostics-and-power-assertions', () => {
    const source = 'result: Int(this > 42) = 42\n';
    const evaluate = power => api.evaluateLanguage(source, 'file:///m2-platform.pkl', 'm2-platform', 'result', power);
    const frames = '\n1 | result: Int(this > 42) = 42\n                ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result: Int(this > 42) = 42\n                             ^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n';
    throws(() => evaluate(undefined), '–– Pkl Error ––\nType constraint `this > 42` violated.\nValue: 42\n' + frames);
    throws(() => evaluate(true), '–– Pkl Error ––\nType constraint `this > 42` violated.\nValue: 42\n\n    this > 42\n    │    │\n    42   false\n' + frames);
    // Enabling tracking must not turn an ordinary type test into an exception.
    eq(api.evaluateLanguage('result = 42 is Int(this < 0)\n', 'file:///m2-platform.pkl', 'm2-platform', 'result', true), 'false');
  });
  // Expected bytes captured from pinned Java at the identical module URI.
  await check('m2-ordered-collection-members-and-amendments', () => {
    eq(api.evaluateLanguage("o = new Dynamic { count = 2; 20; [\"answer\"] = 22 }\nresult = o[0] + o[\"answer\"]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
    eq(api.evaluateLanguage("base = new Listing { 10; 20 }\no = (base) { [0] = 12; 10 }\nresult = o[0] + o[1] + o[2]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
  });
  await check('m2-collection-generators-and-spread', () => {
    eq(api.evaluateLanguage("source = new Listing { 10; 20 }\no = new Listing { for (i, v in source) { (x) -> i + v + x }; ...List(9) }\nresult = o[0].apply(1) + o[1].apply(1) + o[2]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
  });
  await check('m2-collection-diagnostics-and-power-assertions', () => {
    throws(() => api.evaluateLanguage("values = new Listing { 42 }\nresult = values[1]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false),
      "–– Pkl Error ––\nElement index `1` is out of range `0`..`0`.\n\n2 | result = values[1]\n             ^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("values = new Listing { 1; 2 }\nresult: Int(values[0] > 3) = 42\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', true),
      "–– Pkl Error ––\nType constraint `values[0] > 3` violated.\nValue: 42\n\n    values[0] > 3\n    │     │   │\n    │     1   false\n    new Listing { 1; ? }\n\n2 | result: Int(values[0] > 3) = 42\n                ^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n2 | result: Int(values[0] > 3) = 42\n                                 ^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Generic collection expectations use the identical pinned Java module URI.
  await check('m2-lazy-generic-collection-values', () => {
    eq(api.evaluateLanguage("values: Listing<Int> = new Listing { 40; 2; \"unused\" }\nresult = values[0] + values[1]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
    eq(api.evaluateLanguage("values: Mapping<String, Int> = new Mapping { [\"x\"] = 40; [\"y\"] = 2; [\"unused\"] = \"bad\" }\nresult = values[\"x\"] + values[\"y\"]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
  });
  await check('m2-strict-list-nested-wrappers', () => {
    eq(api.evaluateLanguage("values: List<Listing<Int>> = List(new Listing { 42; \"unused\" })\nresult = values[0][0]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
  });
  await check('m2-generic-collection-diagnostics', () => {
    throws(() => api.evaluateLanguage("values: Mapping<Int, Int> = new Mapping { [\"bad\"] = 42 }\nresult = values.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false),
      "–– Pkl Error ––\nExpected value of type `Int`, but got type `String`.\nValue: \"bad\"\n\n1 | values: Mapping<Int, Int> = new Mapping { [\"bad\"] = 42 }\n                    ^^^\nat m2-platform#values (file:///m2-platform.pkl)\n\n1 | values: Mapping<Int, Int> = new Mapping { [\"bad\"] = 42 }\n                                               ^^^^^\nat m2-platform#values[\"bad\"] (file:///m2-platform.pkl)\n\n1 | values: Mapping<Int, Int> = new Mapping { [\"bad\"] = 42 }\n                                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#values (file:///m2-platform.pkl)\n\n2 | result = values.length\n             ^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("values: Listing<Int> = new Listing { 1; 2 }\nresult: Int(values[0] > 3) = 42\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', true),
      "–– Pkl Error ––\nType constraint `values[0] > 3` violated.\nValue: 42\n\n    values[0] > 3\n    │     │   │\n    │     1   false\n    new Listing { 1; 2 }\n\n2 | result: Int(values[0] > 3) = 42\n                ^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n2 | result: Int(values[0] > 3) = 42\n                                 ^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Predicate expectations were captured from pinned Java at this exact URI.
  await check('m2-member-predicate-values', () => {
    eq(api.evaluateLanguage("base = new Listing { new { value = 1 }; new { value = 2 } }\nvalues = (base) { [[value == 2]] { value = 41 } }\nresult = values[0].value + values[1].value\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
    eq(api.evaluateLanguage("base = new Mapping { [\"x\"] = 1; [\"y\"] = 2 }\nvalues = (base) { [[this == 2]] = 41 }\nresult = values[\"x\"] + values[\"y\"]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
    eq(api.evaluateLanguage("base = new Listing { 1; 2 }\nvalues = (base) { [[let (nested = (new Listing { 9 }) { [[this == 9]] = 0 }) nested[0] == 0 && this == 2]] = 42 }\nresult = values[0] + values[1]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "43");
  });
  await check('m2-member-predicate-owner-cache', () => {
    eq(api.evaluateLanguage("base = new Dynamic { marker = 1; [\"k\"] = marker }\nchild = (base) { marker = 2 }\nscanned = (child) { [[false]] = 0 }\nresult = let (ignored = scanned[\"k\"]) base[\"k\"]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "2");
  });
  await check('m2-member-predicate-diagnostics', () => {
    throws(() => api.evaluateLanguage("base = new Listing { 1; 2 }\nvalues = (base) { [[\"bad\"]] = 42 }\nresult = values.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nExpected value of type `Boolean`, but got type `String`.\nValue: \"bad\"\n\n2 | values = (base) { [[\"bad\"]] = 42 }\n                        ^^^^^\nat m2-platform#values (file:///m2-platform.pkl)\n\n3 | result = values.length\n             ^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("base = new Listing { 1; 2 }\nvalues = (base) { [[this > 1]] = 42 }\nresult: Int(values[1] == 3) = 0\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', true), "–– Pkl Error ––\nType constraint `values[1] == 3` violated.\nValue: 0\n\n    values[1] == 3\n    │     │   │\n    │     42  false\n    new Listing { ?; 42 }\n\n3 | result: Int(values[1] == 3) = 0\n                ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n3 | result: Int(values[1] == 3) = 0\n                                  ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Structural comparison expectations use pinned Java at the exact adapter URI.
  await check('m2-structural-equality', () => {
    eq(api.evaluateLanguage("left = new Dynamic { values = new Listing { new { x = 1 }; 2 }; [\"a\"] = 3 }\nright = new Dynamic { [\"a\"] = 3; values = new Listing { new { x = 1 }; 2 } }\nresult = left == right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
    eq(api.evaluateLanguage("result = (1 == 1.0) && (new Dynamic { x = 1 } != new Dynamic { x = 1.0 }) && (List(0.0) != List(-0.0))\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
    eq(api.evaluateLanguage("value = new Listing { throw(\"unforced\") }\nresult = value == value && new Listing { throw(\"unforced\") } != new Listing {}\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
    eq(api.evaluateLanguage("class Item { x: Int; hidden ignored: Int = throw(\"hidden\") }\nleft = new Item { x = 42 }\nright = new Item { x = 42 }\nresult = left == right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
  });
  await check('m2-structural-keys', () => {
    eq(api.evaluateLanguage("values = new Mapping { [new Dynamic { x = 1; y = List(2, 3) }] = 42 }\nresult = values[new Dynamic { y = List(2, 3); x = 1 }]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
    eq(api.evaluateLanguage("base = new Mapping { [new Dynamic { x = 1 }] = 1 }\nvalues = (base) { [new Dynamic { x = 1 }] = 42 }\nresult = values[new Dynamic { x = 1 }]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
    eq(api.evaluateLanguage("values = new Mapping { [List(1)] = 20; [List(1.0)] = 22 }\nresult = values[List(1)] + values[List(1.0)]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
  });
  await check('m2-structural-equality-diagnostics', () => {
    throws(() => api.evaluateLanguage("values = new Mapping { [new Dynamic { x = 1 }] = 1; [new Dynamic { x = 1 }] = 2 }\nresult = values.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nDuplicate definition of member `new Dynamic { x = 1 }`.\n\n1 | values = new Mapping { [new Dynamic { x = 1 }] = 1; [new Dynamic { x = 1 }] = 2 }\n                                                         ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#values (file:///m2-platform.pkl)\n\n2 | result = values.length\n             ^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("left = new { x = new { fail = throw(\"left nested\") } }\nright = new { x = 2 }\nresult = left == right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nleft nested\n\n1 | left = new { x = new { fail = throw(\"left nested\") } }\n                                  ^^^^^^^^^^^^^^^^^^^^\nat m2-platform#left.x.fail (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("left = new Listing { 1; 2 }\nright = new Listing { 1; 3 }\nresult: Int(left == right) = 0\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', true), "–– Pkl Error ––\nType constraint `left == right` violated.\nValue: 0\n\n    left == right\n    │    │  │\n    │    │  new Listing { 1; 3 }\n    │    false\n    new Listing { 1; 2 }\n\n3 | result: Int(left == right) = 0\n                ^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n3 | result: Int(left == right) = 0\n                                 ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Stage 8 expectations are pinned Java captures at this exact module URI.
  await check('m2-typed-default-equality-and-keys', () => {
    eq(api.evaluateLanguage("class C { default: Int = 1 }; result = new C {} == new C { default = 2 }\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "false");
    eq(api.evaluateLanguage("class C { default: Int }; result = new Mapping { [new C {default=1}]=10; [new C {default=2}]=20 }[new C {default=2}]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "20");
    throws(() => api.evaluateLanguage("class C { default: Int }; result = new C { default = throw(\"left default\") } == new C { default = throw(\"right default\") }\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nleft default\n\n1 | class C { default: Int }; result = new C { default = throw(\"left default\") } == new C { default = throw(\"right default\") }\n                                                         ^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.default (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  await check('m2-comparison-cache-and-class-order', () => {
    throws(() => api.evaluateLanguage("left = new Dynamic { a=0; b=new Mapping { [new Dynamic { key=throw(\"cached b first\") }]=1 } }; right = new Dynamic { a=1; b=new Mapping { [new Dynamic { key=1 }]=1 } }; result = (left.b == left.b) && left == right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\ncached b first\n\n1 | left = new Dynamic { a=0; b=new Mapping { [new Dynamic { key=throw(\"cached b first\") }]=1 } }; right = new Dynamic { a=1; b=new Mapping { [new Dynamic { key=1 }]=1 } }; result = (left.b == left.b) && left == right\n                                                                 ^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#left.b.key (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    eq(api.evaluateLanguage("left = new Dynamic { a=0; b=new Mapping { [new Dynamic { key=throw(\"cached b first\") }]=1 } }; right = new Dynamic { a=1; b=new Mapping { [new Dynamic { key=1 }]=1 } }; result = left == right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "false");
    eq(api.evaluateLanguage("class C { a: Int; b: Mapping }; left=new C {b=new Mapping {[new Dynamic {key=throw(\"skip reordered b\")}]=1};a=0};right=new C {b=new Mapping {[new Dynamic {key=1}]=1};a=1};result=left==right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "false");
  });
  await check('m2-deep-force-before-key-hash', () => {
    throws(() => api.evaluateLanguage("result=let(m=new Mapping {[new Dynamic { first=new Mapping {[new Dynamic {x=throw(\"nested hash\")}]=1};later=throw(\"deep force first\") }]=0;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}) 1\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\ndeep force first\n\n1 | result=let(m=new Mapping {[new Dynamic { first=new Mapping {[new Dynamic {x=throw(\"nested hash\")}]=1};later=throw(\"deep force first\") }]=0;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}) 1\n                                                                                                                ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.later (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Stage 9 expectations are raw pinned Java captures at this exact URI.
  await check('m2-local-cache-comparison', () => {
    eq(api.evaluateLanguage("left=new Listing {local x=1+0;x};right=new Listing {1};result=left==right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "false");
    eq(api.evaluateLanguage("left=new Listing {local x=1+0;x};right=new Listing {1};result=right==left\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
    eq(api.evaluateLanguage("left=new Mapping {local x=1+0;[\"a\"]=x};right=new Mapping {[\"a\"]=1};result=left==right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "false");
    eq(api.evaluateLanguage("left=new Mapping {local x=1+0;[\"a\"]=x};right=new Mapping {[\"a\"]=1};result=right==left\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
    eq(api.evaluateLanguage("left=new Dynamic {local x=1+0;a=x};right=new Dynamic {a=1};result=left==right\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
  });
  await check('m2-hashed-key-buckets', () => {
    eq(api.evaluateLanguage("left=new Listing {1;17};right=new Listing {1;17};p=new Mapping {[left]=0};c=(p){[right]=7};result=c.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "1");
    eq(api.evaluateLanguage("left=new Listing {1;17};right=new Listing {1;17};p=new Mapping {[left]=0};c=(p){[right]=7};result=let(wleft=left[1]) c.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "2");
    eq(api.evaluateLanguage("left=new Listing {local x=0+0;x};right=new Listing {0};p=new Mapping {[left]=0};c=(p){[right]=7};result=c.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "1");
    eq(api.evaluateLanguage("left=new Listing {local x=0+0;x};right=new Listing {0};p=new Mapping {[right]=0};c=(p){[left]=7};result=c.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "2");
    eq(api.evaluateLanguage("p=new Mapping {[new Dynamic {x=1}]=0};c=(p){[new Dynamic {x=1}]=7};result=c.length\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "1");
    throws(() => api.evaluateLanguage("key=new Listing {local x=1+0;default=(_)->x;0};result=let(index=new Mapping {[key]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}) let(warm=key.default.apply(0)) index[key]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nCannot find key `new Listing { 0 }`.\n\n1 | key=new Listing {local x=1+0;default=(_)->x;0};result=let(index=new Mapping {[key]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}) let(warm=key.default.apply(0)) index[key]\n                                                                                                                                                           ^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | key=new Listing {local x=1+0;default=(_)->x;0};result=let(index=new Mapping {[key]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}) let(warm=key.default.apply(0)) index[key]\n                                                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr> (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    eq(api.evaluateLanguage("key=new Listing {local x=1+0;default=(_)->x;2};result=let(index=new Mapping {[key]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}) let(warm=key.default.apply(0)) index[key]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "10");
    eq(api.evaluateLanguage("m=new Mapping {[0.0]=0;[-0.0]=1;[NaN]=2;[Infinity]=3;[-Infinity]=4;[1.0]=5;};result=m[0.0]==0 && m[-0.0]==1 && m[NaN]==2 && m[Infinity]==3 && m[-Infinity]==4 && m[1.0]==5\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
    eq(api.evaluateLanguage("result=new Mapping {[List(0)]=0;[List(32)]=1;[List(64)]=2;[List(96)]=3;[List(128)]=4;[List(160)]=5;[List(192)]=6;[List(224)]=7;[List(256)]=8;[List(288)]=9;[List(320)]=10;[List(352)]=11;[List(384)]=12;[List(416)]=13;[List(448)]=14;[List(480)]=15;[List(512)]=16;[List(544)]=17;[List(576)]=18;[List(608)]=19;[List(640)]=20;[List(672)]=21;[List(704)]=22;}[List(704)]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "22");
    eq(api.evaluateLanguage("left=new Listing {1;17};right=new Listing {1;17};result=let(w=left[1]) new Mapping {[left]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}[right]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "10");
    throws(() => api.evaluateLanguage("left=new Listing {1;2};right=new Listing {1;2};result=let(w=left[1]) new Mapping {[left]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}[right]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nCannot find key `new Listing { 1; 2 }`.\n\n1 | left=new Listing {1;2};right=new Listing {1;2};result=let(w=left[1]) new Mapping {[left]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}[right]\n                                                                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | left=new Listing {1;2};right=new Listing {1;2};result=let(w=left[1]) new Mapping {[left]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4}[right]\n                                                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr> (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    eq(api.evaluateLanguage("left=new Listing {1;-15};right=new Listing {1;-15};result=let(w=left[1]) let(m=new Mapping {[left]=10;[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4;[right]=20}) m[right]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "20");
  });
  await check('m2-cache-fifth-publication', () => {
    eq(api.evaluateLanguage("key=new Listing {throw(\"cache hash\")};p=new Mapping {[key]=9};c=(p) {[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4};result=List(c[key],c[\"a\"],c[\"b\"],c[\"c\"])[3]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "3");
    throws(() => api.evaluateLanguage("key=new Listing {throw(\"cache hash\")};p=new Mapping {[key]=9};c=(p) {[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4};result=List(c[\"a\"],c[\"b\"],c[key],c[\"c\"],c[\"d\"])[4]\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\ncache hash\n\n1 | key=new Listing {throw(\"cache hash\")};p=new Mapping {[key]=9};c=(p) {[\"a\"]=1;[\"b\"]=2;[\"c\"]=3;[\"d\"]=4};result=List(c[\"a\"],c[\"b\"],c[key],c[\"c\"],c[\"d\"])[4]\n                     ^^^^^^^^^^^^^^^^^^^\nat m2-platform#key[#1] (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  await check('m2-let-diagnostic-scopes', () => {
    throws(() => api.evaluateLanguage("result: Int(let(_=1) throw(\"wildcard\"))=0\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nwildcard\n\n1 | result: Int(let(_=1) throw(\"wildcard\"))=0\n                         ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result: Int(let(_=1) throw(\"wildcard\"))=0\n                                            ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("result=let(x=1) (0 as Int(let(_=this) throw(\"wildcard\")))\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nwildcard\n\n1 | result=let(x=1) (0 as Int(let(_=this) throw(\"wildcard\")))\n                                          ^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result=let(x=1) (0 as Int(let(_=this) throw(\"wildcard\")))\n           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr> (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("result: Int(let(_=1) let(x=this) throw(\"named\"))=0\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nnamed\n\n1 | result: Int(let(_=1) let(x=this) throw(\"named\"))=0\n                                     ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result: Int(let(_=1) let(x=this) throw(\"named\"))=0\n                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr> (file:///m2-platform.pkl)\n\n1 | result: Int(let(_=1) let(x=this) throw(\"named\"))=0\n                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result: Int(let(_=1) let(x=this) throw(\"named\"))=0\n                                                     ^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("result=let(x=1) new Dynamic { y=let(z=2) throw(\"member\") }.y\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nmember\n\n1 | result=let(x=1) new Dynamic { y=let(z=2) throw(\"member\") }.y\n                                             ^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr>.y (file:///m2-platform.pkl)\n\n1 | result=let(x=1) new Dynamic { y=let(z=2) throw(\"member\") }.y\n                                    ^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr>.y.<let expr> (file:///m2-platform.pkl)\n\n1 | result=let(x=1) new Dynamic { y=let(z=2) throw(\"member\") }.y\n                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result=let(x=1) new Dynamic { y=let(z=2) throw(\"member\") }.y\n           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result.<let expr> (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("result=let(x=throw(\"initializer\")) x\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\ninitializer\n\n1 | result=let(x=throw(\"initializer\")) x\n                 ^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("module demo.lets\nresult = let (message = \"😀\")\n  throw(message)\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\n😀\n\n3 | throw(message)\n    ^^^^^^^^^^^^^^\nat demo.lets#result (file:///m2-platform.pkl)\n\n2 | result = let (message = \"😀\")\n             ^^^^^^^^^^^^^^^^^^^^\nat demo.lets#result.<let expr> (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Stage 10 expectations are complete pinned Java captures at this exact URI.
  await check('m2-reference-paths-and-rendering', () => {
    eq(api.evaluateLanguage("import \"pkl:ref\" as deferred\nclass D extends deferred.Domain {\n  function renderReference(r): String = r.getData() + \"/\" + r.getPath()[0].property + \"[\" + r.getPath()[1].key + \"]\"\n}\nclass Holder { values: Mapping<String, String> = throw(\"referent must stay deferred\") }\nlocal r = deferred.Reference(new D {}, Holder, \"root\").values[\"x\"]\nresult = \"resolved: \\(r)\"\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "resolved: root/values[x]");
  });
  await check('m2-reference-equality-and-type-checks', () => {
    eq(api.evaluateLanguage("import \"pkl:ref\"\nclass D extends ref.Domain { function renderReference(r): String = throw(\"comparison must not render\") }\nclass A { value: String(length > 0) = throw(\"left referent must stay deferred\") }\nclass B { value: String = throw(\"right referent must stay deferred\") }\nlocal d = new D {}\nlocal left = ref.Reference(d, A, 42).value\nlocal right = ref.Reference(d, B, 42).value\nresult = left == right && left is ref.Reference<D, String> && left.getPath()[0].isProperty\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
  });
  await check('m2-reference-stdlib-diagnostics', () => {
    throws(() => api.evaluateLanguage("import \"pkl:ref\"\nresult = ref.Reference(42, String, null)\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nType constraint `this is Domain` violated.\nValue: 42\n\n49 | domain: D(this is Domain),\n               ^^^^^^^^^^^^^^\nat pkl.ref#Reference (https://github.com/apple/pkl/blob/0.32.0/stdlib/ref.pkl#L49)\n\n2 | result = ref.Reference(42, String, null)\n             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("import \"pkl:ref\"\nclass D extends ref.Domain { function renderReference(r): String = throw(\"domain render failed\") }\nresult = ref.Reference(new D {}, String, null).toString()\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\ndomain render failed\n\n2 | class D extends ref.Domain { function renderReference(r): String = throw(\"domain render failed\") }\n                                                                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#D.renderReference (file:///m2-platform.pkl)\n\n201 | function toString(): String = getDomain().renderReference(this)\n                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat pkl.ref#Reference.toString (https://github.com/apple/pkl/blob/0.32.0/stdlib/ref.pkl#L201)\n\n3 | result = ref.Reference(new D {}, String, null).toString()\n             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
  });
  // Complete pinned Java captures: target/m2/stage11-platform-java-01.
  await check("m2-fixed-inheritance-and-virtual-receivers", () => {
    eq(api.evaluateLanguage("open class Base { fixed value: Int = offset + 1; offset: Int = 1 }\nclass Child extends Base { offset = 40 }\nlocal child = new Child {}\nlocal amended = (child) { offset = 41 }\nresult = amended.value\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "42");
  });
  // Complete pinned Java captures: target/m2/stage11-platform-java-01.
  await check("m2-fixed-reference-declaration-metadata", () => {
    eq(api.evaluateLanguage("import \"pkl:ref\"\nclass D extends ref.Domain { function renderReference(r): String = throw(\"render must stay deferred\") }\nopen class Base { hidden fixed value: String = throw(\"referent must stay deferred\") }\nclass Child extends Base {}\nlocal r = ref.Reference(new D {}, Child, 42).value\nresult = r is ref.Reference<D, String> && r.getPath()[0].property == \"value\" && r.getData() == 42\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "true");
  });
  // Complete pinned Java captures: target/m2/stage11-platform-java-01.
  await check("m2-fixed-property-diagnostics", () => {
    throws(() => api.evaluateLanguage("class C { fixed value: Int = 42 }\nresult = new C { value = 1 }.value\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nCannot assign to fixed property `value`.\n\n2 | result = new C { value = 1 }.value\n                     ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("open class Base { fixed value: Int = 42 }\nclass Child extends Base { value: Int = 1 }\nresult = new Child {}.value\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nMissing modifier `fixed`.\n\n2 | class Child extends Base { value: Int = 1 }\n                               ^^^^^^^^^^\nat m2-platform#Child (file:///m2-platform.pkl)\n\nProperty `value` must be declared fixed, because it overrides a fixed property on parent class `m2-platform#Base`.\n\n3 | result = new Child {}.value\n                 ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n");
    throws(() => api.evaluateLanguage("abstract class C { abstract fixed value: Int = 42 }\nresult = 42\n", 'file:///m2-platform.pkl', 'm2-platform', 'result', false), "–– Pkl Error ––\nAbstract members cannot have a body.\n\n1 | abstract class C { abstract fixed value: Int = 42 }\n                       ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#C.value (file:///m2-platform.pkl)\n");
  });
  await check('utf8-utf16-string-boundary', () => {
    for (const value of ['', '한글🙂e\u0301\0\r\n', '𐐀漢字']) eq(api.roundtripString(value), value);
    for (const value of ['\ud800', '\udfff', 'a\ud800b']) throws(() => api.roundtripString(value), 'unpaired UTF-16 surrogate is unsupported');
    for (const value of [123, null, undefined, {}]) throws(() => api.roundtripString(value));
  });
  await check('lossless-i64-bigint-range', () => {
    for (const value of [0n, -1n, 9007199254740993n, -(1n << 63n), (1n << 63n) - 1n]) eq(api.roundtripInteger(value), value);
    for (const value of [1n << 63n, -(1n << 63n) - 1n, 1n << 100n, -(1n << 100n)]) throws(() => api.roundtripInteger(value), 'BigInt is outside signed 64-bit range');
    throws(() => api.roundtripInteger(42));
  });
  await check('owned-byte-array-boundary', () => {
    const input = new Uint8Array([0, 255, 13, 10, 128, 42]);
    const result = api.roundtripBytes(input);
    eq([...result].join(','), [...input].join(','));
    result[0] = 99; eq(input[0], 0);
    input[1] = 7; eq(result[1], 255);
    for (const value of [[], {}, new Uint16Array([1]), null]) throws(() => api.roundtripBytes(value));
  });
  await check('same-evaluator-nested-read-resume', async () => {
    const probe = new api.HostProbe('add');
    const host = new FakeHostOperations(probe, clock);
    try {
      probe.start(clock(), 10000); const token = read(probe);
      const outer = host.begin(token);
      eq(read(probe), token);
      probe.reenter(token, clock(), 10000);
      const nestedToken = read(probe);
      assert(nestedToken !== token, 'nested read reused parent identity');
      const nested = host.begin(nestedToken);
      eq(host.stats.active, 2); eq(host.stats.resources, 2);
      throws(() => probe.respondInteger(token, 9n, clock()), 'stale or unexpected Host completion');
      await delay(5);
      host.complete(nestedToken, 2n);
      eq((await nested.settled).value, 2n);
      eq(probe.poll(clock(), 100), 'ready\tint\t102');
      eq(read(probe), token);
      eq(host.stats.active, 1); eq(host.stats.resources, 1);
      host.complete(token, 3n);
      eq((await outer.settled).value, 3n);
      throws(() => probe.respondInteger(token, 3n, clock()), 'stale or unexpected Host completion');
      eq(probe.poll(clock(), 100), 'ready\tint\t43');
      eq(host.stats.resources, 0); eq(host.stats.settled, 2); eq(host.stats.aborted, 0);
      eq(host.operations.size, 0); eq(host.stats.timers, 0); eq(host.stats.listeners, 0);
      probe.start(clock(), 10000); const next = read(probe);
      assert(next !== token, 'request identity reused');
      throws(() => probe.respondInteger(token, 2n, clock()), 'stale or unexpected Host completion');
      probe.respondInteger(next, 3n, clock());
      eq(probe.poll(clock(), 100), 'ready\tint\t43');
    } finally { probe.close(); host.drainAborts(); if (probe.free) probe.free(); }
  });
  await check('cross-session-completion-rejected', () => {
    const a = new api.HostProbe('read'), b = new api.HostProbe('read');
    try {
      a.start(clock(), 10000); b.start(clock(), 10000);
      const at = read(a), bt = read(b);
      throws(() => b.respondInteger(at, 1n, clock()), 'stale or unexpected Host completion');
      b.respondInteger(bt, 5n, clock()); eq(b.poll(clock(), 100), 'ready\tint\t5');
    } finally { dispose(a); dispose(b); }
  });
  await check('async-resource-bytes', async () => {
    const probe = new api.HostProbe('read');
    try {
      probe.start(clock(), 10000); const token = read(probe);
      await delay(5);
      probe.respondBytes(token, new Uint8Array([0, 255, 13, 10]), clock());
      eq(probe.poll(clock(), 100), 'ready\tbytes\t00ff0d0a');
    } finally { dispose(probe); }
  });
  await check('host-error-bytes', async () => {
    const probe = new api.HostProbe('read');
    try {
      probe.start(clock(), 10000); const token = read(probe);
      await delay(5); probe.reject(token, 'failed\0\r\n🙂', clock());
      eq(probe.poll(clock(), 100), 'error\tHost\t6661696c6564000d0af09f9982');
    } finally { dispose(probe); }
  });
  for (const action of ['cancel', 'close', 'timeout']) {
    await check(`live-host-${action}-abort-release-settle`, async () => {
      const probe = new api.HostProbe('read');
      let evaluation;
      try {
        evaluation = waitingEvaluation(probe, action === 'timeout' ? 50 : 10000, clock);
        const { host } = evaluation;
        const [operation] = host.operations.values();
        eq(host.stats.active, 1); eq(host.stats.resources, 1);
        assert(!operation.controller.signal.aborted, 'operation aborted before terminal action');
        await delay(5);
        eq(host.stats.active, 1); eq(host.stats.settled, 0);
        if (action !== 'timeout') { probe[action](); evaluation.pump(); }
        const result = await evaluation.result;
        eq(result.split('\t')[1], { cancel: 'Cancelled', close: 'Closed', timeout: 'Timeout' }[action]);
        eq((await operation.settled).error, 'AbortError');
        assert(operation.controller.signal.aborted, 'underlying Host operation was not aborted');
        eq(host.stats.aborted, 1); eq(host.stats.active, 0); eq(host.stats.resources, 0);
        eq(host.operations.size, 0); eq(host.stats.timers, 0); eq(host.stats.listeners, 0);
        eq(host.stats.settled, 1); eq(host.stats.rejected, 1); eq(host.stats.fulfilled, 0);
        eq(probe.drainCancelledReads().length, 0);
        throws(() => host.complete(operation.token, 1n), 'evaluator is closed');
        eq(host.stats.lateRejected, 1); eq(host.stats.settled, 1);
        throws(() => probe.start(clock(), 10000), 'evaluator is closed');
      } finally {
        if (evaluation) { evaluation.stop(); probe.close(); evaluation.host.drainAborts(); }
        dispose(probe);
      }
    });
  }
  await check('nested-terminal-aborts-all-live-host-operations', async () => {
    for (const action of ['cancel', 'close', 'timeout']) {
      const probe = new api.HostProbe('add');
      let evaluation;
      try {
        // The outer deadline also bounds a nested context with a longer deadline.
        evaluation = waitingEvaluation(probe, action === 'timeout' ? 50 : 10000, clock);
        const { host } = evaluation;
        const [outer] = host.operations.values();
        probe.reenter(outer.token, clock(), 10000);
        evaluation.pump();
        const operations = [...host.operations.values()];
        eq(operations.length, 2); eq(host.stats.active, 2); eq(host.stats.resources, 2);
        eq(host.stats.timers, 2); eq(host.stats.listeners, 2);
        await delay(5);
        eq(host.stats.settled, 0);
        if (action !== 'timeout') { probe[action](); evaluation.pump(); }
        eq((await evaluation.result).split('\t')[1], { cancel: 'Cancelled', close: 'Closed', timeout: 'Timeout' }[action]);
        for (const operation of operations) {
          eq((await operation.settled).error, 'AbortError');
          assert(operation.controller.signal.aborted, 'nested terminal left an active Host operation');
          throws(() => host.complete(operation.token, 1n), 'evaluator is closed');
        }
        eq(host.stats.aborted, 2); eq(host.stats.rejected, 2); eq(host.stats.settled, 2);
        eq(host.stats.fulfilled, 0); eq(host.stats.lateRejected, 2);
        eq(host.stats.active, 0); eq(host.stats.resources, 0); eq(host.operations.size, 0);
        eq(host.stats.timers, 0); eq(host.stats.listeners, 0);
        eq(probe.drainCancelledReads().length, 0);
        throws(() => probe.start(clock(), 10000), 'evaluator is closed');
      } finally {
        if (evaluation) { evaluation.stop(); probe.close(); evaluation.host.drainAborts(); }
        dispose(probe);
      }
    }
  });
  await check('cooperative-loop-cancel', async () => {
    const probe = new api.HostProbe('loop');
    try {
      probe.start(clock(), 10000); eq(probe.poll(clock(), 1000), 'yield');
      await delay(5); probe.cancel();
      eq(probe.poll(clock(), 100).split('\t')[1], 'Cancelled');
    } finally { dispose(probe); }
  });
  await check('invalid-clock-budget-boundary', () => {
    const probe = new api.HostProbe('loop');
    try {
      for (const value of [-1, 0.5, NaN, Infinity, 4294967296]) {
        const message = 'expected an integer in 0..=4294967295';
        throws(() => probe.start(value, 100), message);
        throws(() => probe.start(0, value), message);
        throws(() => probe.poll(value, 100), message);
        throws(() => probe.poll(0, value), message);
      }
      probe.start(4294967295, 1);
      eq(probe.poll(4294967295, 1), 'yield');
    } finally { dispose(probe); }
  });
  return passed;
}

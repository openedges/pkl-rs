//! Full raw errors from pinned Stage38 generic method-lookup Java controls.
use pkl_runtime::evaluation::scalar_expression;

#[test]
fn implicit_scopes_and_explicit_receiver_diagnostics_match_java() {
    let cases = [
        (
            "implicit-empty",
            "result=absent(throw(\"argument\"))\n",
            "–– Pkl Error ––\nCannot find method `absent`.\n\n1 | result=absent(throw(\"argument\"))\n           ^^^^^^^^^^^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-nonempty",
            "function zeta(x,y)=x\nresult=absent(1)\n",
            "–– Pkl Error ––\nCannot find method `absent`.\n\n2 | result=absent(1)\n           ^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-typo-method",
            "function calculate(x)=x\nresult=calclate(1)\n",
            "–– Pkl Error ––\nCannot find method `calclate`.\n\n2 | result=calclate(1)\n           ^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\ncalculate(x)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-typo-arity",
            "function calculate(x)=x\nresult=calclate(1,2)\n",
            "–– Pkl Error ––\nCannot find method `calclate`.\n\n2 | result=calclate(1,2)\n           ^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\ncalculate(x)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-property",
            "value=1\nresult=value()\n",
            "–– Pkl Error ––\nCannot find method `value`.\n\n2 | result=value()\n           ^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nvalue\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-local-method",
            "local function calculate(x)=x\nresult=calclate(1)\n",
            "–– Pkl Error ––\nCannot find method `calclate`.\n\n2 | result=calclate(1)\n           ^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\ncalculate(x)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-inherited-method",
            "result=toStrng()\n",
            "–– Pkl Error ––\nCannot find method `toStrng`.\n\n1 | result=toStrng()\n           ^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\ntoString()\nString\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-class-method",
            "class C {function nearby(x)=x; answer=nearb(1)}\nresult=new C {}.answer\n",
            "–– Pkl Error ––\nCannot find method `nearb`.\n\n1 | class C {function nearby(x)=x; answer=nearb(1)}\n                                          ^^^^^^^^\nat stage38-read-frontend#C.answer (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nnearby(x)\n\n2 | result=new C {}.answer\n           ^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-class-empty",
            "class C {answer=absent(1)}\nresult=new C {}.answer\n",
            "–– Pkl Error ––\nCannot find method `absent`.\n\n1 | class C {answer=absent(1)}\n                    ^^^^^^^^^\nat stage38-read-frontend#C.answer (file:///stage38-read-frontend.pkl)\n\n2 | result=new C {}.answer\n           ^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "explicit-module-empty",
            "result=module.absent(1)\n",
            "–– Pkl Error ––\nCannot find method `absent` in module `stage38-read-frontend`.\n\n1 | result=module.absent(1)\n           ^^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nAvailable methods in module `stage38-read-frontend`:\ngetClass\ngetProperty\ngetPropertyOrNull\nhasProperty\nifNonNull\nrelativePathTo\ntoDynamic\ntoMap\ntoString\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "explicit-module-nonempty",
            "function zeta(x,y)=x\nresult=module.absent(1)\n",
            "–– Pkl Error ––\nCannot find method `absent` in module `stage38-read-frontend`.\n\n2 | result=module.absent(1)\n           ^^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nAvailable methods in module `stage38-read-frontend`:\ngetClass\ngetProperty\ngetPropertyOrNull\nhasProperty\nifNonNull\nrelativePathTo\ntoDynamic\ntoMap\ntoString\nzeta\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "implicit-base-constructor",
            "result=Lits(1)\n",
            "–– Pkl Error ––\nCannot find method `Lits`.\n\n1 | result=Lits(1)\n           ^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nList(elements)\nList\nListing\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
    ];
    for (id, source, expected) in cases {
        assert_eq!(
            scalar_expression(
                source,
                "file:///stage38-read-frontend.pkl",
                "stage38-read-frontend",
                "result"
            ),
            Err(expected.into()),
            "{id}"
        );
    }
}

#[test]
fn suggestion_rank_ties_preserve_lexical_scope_and_member_kind_order() {
    let cases = [
        (
            "lexical-method-base-property-tie",
            "function Listing(x,y,z)=x\nresult=Listin(1)\n",
            "–– Pkl Error ––\nCannot find method `Listin`.\n\n2 | result=Listin(1)\n           ^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nListing(x, y, z)\nListing\nList(elements)\nList\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "local-method-base-property-tie",
            "local function Listing(x,y,z)=x\nresult=Listin(1)\n",
            "–– Pkl Error ––\nCannot find method `Listin`.\n\n2 | result=Listin(1)\n           ^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nListing(x, y, z)\nListing\nList(elements)\nList\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "outer-method-base-property-tie",
            "function Listing(x,y,z)=x\nclass C { answer=Listin(1) }\nresult=new C {}.answer\n",
            "–– Pkl Error ––\nCannot find method `Listin`.\n\n2 | class C { answer=Listin(1) }\n                     ^^^^^^^^^\nat stage38-read-frontend#C.answer (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nListing(x, y, z)\nListing\nList(elements)\nList\n\n3 | result=new C {}.answer\n           ^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "nearest-method-shadow-tie",
            "function Listing(x,y,z)=x\nclass C {function Listing(a,b,c)=a; answer=Listin(1)}\nresult=new C {}.answer\n",
            "–– Pkl Error ––\nCannot find method `Listin`.\n\n2 | class C {function Listing(a,b,c)=a; answer=Listin(1)}\n                                               ^^^^^^^^^\nat stage38-read-frontend#C.answer (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nListing(a, b, c)\nListing\nList(elements)\nList\n\n3 | result=new C {}.answer\n           ^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "equal-similarity-arity-rank",
            "function hello(a,b,c)=a\nfunction hallo(x)=x\nresult=hillo(1)\n",
            "–– Pkl Error ––\nCannot find method `hillo`.\n\n3 | result=hillo(1)\n           ^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nhallo(x)\nhello(a, b, c)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "equal-rank-declaration-order",
            "function hello(a,b,c)=a\nfunction hallo(x,y,z)=x\nresult=hillo(1)\n",
            "–– Pkl Error ––\nCannot find method `hillo`.\n\n3 | result=hillo(1)\n           ^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nhello(a, b, c)\nhallo(x, y, z)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "explicit-module-excludes-base-properties",
            "function Listing(x,y,z)=x\nresult=module.Listin(1)\n",
            "–– Pkl Error ––\nCannot find method `Listin` in module `stage38-read-frontend`.\n\n2 | result=module.Listin(1)\n           ^^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nListing(x, y, z)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "same-scope-property-before-method",
            "function hallo(a,b,c)=a\nhello=1\nresult=hillo(1)\n",
            "–– Pkl Error ––\nCannot find method `hillo`.\n\n3 | result=hillo(1)\n           ^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nhello\nhallo(a, b, c)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "nested-method-before-outer-property",
            "hello=1\nclass C {function hallo(a,b,c)=a; answer=hillo(1)}\nresult=new C {}.answer\n",
            "–– Pkl Error ––\nCannot find method `hillo`.\n\n2 | class C {function hallo(a,b,c)=a; answer=hillo(1)}\n                                             ^^^^^^^^\nat stage38-read-frontend#C.answer (file:///stage38-read-frontend.pkl)\n\nDid you mean any of the following?\nhallo(a, b, c)\nhello\n\n3 | result=new C {}.answer\n           ^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
    ];
    for (id, source, expected) in cases {
        assert_eq!(
            scalar_expression(
                source,
                "file:///stage38-read-frontend.pkl",
                "stage38-read-frontend",
                "result"
            ),
            Err(expected.into()),
            "{id}"
        );
    }
}

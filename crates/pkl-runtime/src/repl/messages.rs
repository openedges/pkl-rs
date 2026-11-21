//! Exact Pkl 0.32.0 REPL text constants mirroring `org.pkl.cli.repl.ReplMessages`.

/// Welcome banner printed once at REPL startup (`ReplMessages.welcome`).
pub const WELCOME: &str = "Welcome to Pkl 0.32.0.\nType an expression to have it evaluated.\nType `:help` or `:examples` for more information.";

/// Output of `:help` (`ReplMessages.help`, includes trailing newline matching Kotlin `trimIndent()`).
pub const HELP: &str = "\
`<expr>`           Evaluate <expr> and print the result. `1 + 3`
`<name> = <expr>`  Evaluate <expr> and assign the result to property <name>. `msg = \"howdy\"`
`:clear`           Clear the screen.
`:examples`        Show code examples (use copy and paste to run them).
`:force <expr>`    Force eager evaluation of a value.
`:help`            Show this help.
`:load <file>`     Load <file> from local file system. `:load path/to/config.pkl`
`:quit`            Quit this program.
`:reset`           Reset the environment to its initial state.

Tips:
* Commands can be abbreviated. `:h`
* Commands can be completed. `:<TAB>`
* File paths can be completed. `:load <TAB>`
* Expressions can be completed. `\"hello\".re<TAB>`
* Multiple declarations and expressions can be evaluated at once. `a = 1; b = a + 2`
* Incomplete input will be continued on the next line.
* Multi-line programs can be copy-pasted into the REPL.
";

/// Output of `:examples` (`ReplMessages.examples`), matching platform-specific macOS hint
/// and `Release.current().documentation().homepage()` for Pkl 0.32.0.
pub fn examples(is_macos: bool) -> String {
    let mac_hint = if is_macos {
        " (Command+Double-click the link below)"
    } else {
        ""
    };
    format!(
        "\
Expressions:
`2 + 3 * 4`

Strings:
`\"Hello, \" + \"World!\"`

Properties:
`timeout = 5.min; timeout`

Objects:
```pigeon {{
  name = \"Pigeon\"
  fullName = \"\\(name) Bird\"
  age = 42
  address {{
    street = \"Landers St.\"
  }}
}}
pigeon.fullName

hobbies {{
  \"Swimming\"
  \"Dancing\"
  \"Surfing\"
}}
hobbies[1]

prices {{
  [\"Apple\"] = 1.5
  [\"Orange\"] = 5
  [\"Banana\"] = 2
}}
prices[\"Banana\"]```

Inheritance:
```parrot = (pigeon) {{
  name = \"Parrot\"
  age = 41
}}
:force parrot```

For more examples, see the Language Reference{mac_hint}:

https://pkl-lang.org/main/0.32.0/language-reference/
"
    )
}

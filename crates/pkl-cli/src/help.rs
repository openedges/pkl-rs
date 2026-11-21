//! Verbatim Apple Pkl 0.32.0 CLI help texts and Clikt shell-completion scripts.

pub const HELP_ROOT: &str = "\
Usage: pkl [<options>] <command> [<args>]...

Options:
  -v, --version  Show the version and exit
  -h, --help     Show this message and exit

Commands:
  eval              Render pkl module(s)
  repl              Start a REPL session
  server            Run as a server that communicates over standard input/output
  test              Run tests within the given module(s)
  project           Run commands related to projects
  download-package  Download package(s)
  analyze           Commands related to static analysis
  format            Format or check formatting of Pkl files.
  run               Run a Pkl pkl:Command CLI tool
  shell-completion  Generate a completion script for the given shell

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_EVAL: &str = "\
Usage: pkl eval [<options>] <modules>...

  Render pkl module(s)

Options:
  --allowed-modules=<pattern1,pattern2>
                                 URI patterns that determine which modules can
                                 be loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                                 URI patterns that determine which external
                                 resources can be read.
  --root-dir=<path>              Restricts access to file-based modules and
                                 resources to those located under the root
                                 directory.
  --cache-dir=<path>             The cache directory for storing packages.
  -w, --working-dir=<path>       Base path that relative module paths are
                                 resolved against.
  -p, --property=<name=value>    External property to set (repeatable).
  --color=<when>                 Whether to format messages in ANSI color.
                                 Possible values of <when> are 'never', 'auto',
                                 and 'always'.
  --no-cache                     Disable caching of packages
  -f, --format=<text>            Output format to generate. <json, jsonnet, pcf,
                                 properties, plist, textproto, xml, yaml,
                                 pkl-binary>
  -e, --env-var=<name=value>     Environment variable to set (repeatable).
  --module-path=<path1:path2>    Directories, ZIP archives, or JAR archives to
                                 search when resolving modulepath: URIs.
  --settings=<value>             Pkl settings module to use.
  -t, --timeout=<number>         Duration, in seconds, after which evaluation of
                                 a source module will be timed out.
  --ca-certificates=<path>       Only trust CA certificates from the provided
                                 file(s).
  --http-proxy=<address>         Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                                 Hostnames that should not be connected to via a
                                 proxy.
  --http-rewrite=<from=to>       URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                                 HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                                 External reader registrations for module URI
                                 schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                                 External reader registrations for resource URI
                                 schemes
  --trace-mode=<style>           Specifies how calls to trace() are formatted.
                                 Possible values of <style> are 'compact' and
                                 'pretty'.
  --project-dir=<path>           The project directory to use for this command.
                                 By default, searches up from the working
                                 directory for a PklProject file.
  --omit-project-settings        Ignores evaluator settings set in the
                                 PklProject file.
  --no-project                   Disables loading settings and dependencies from
                                 the PklProject file.
  -o, --output-path=<path>       File path where the output file is placed.
  --module-output-separator=<string>
                                 Separator to use when multiple module outputs
                                 are written to the same file. (default: ---)
  -x, --expression=<expression>  Expression to be evaluated within the module.
  -m, --multiple-file-output-path=<path>
                                 Directory where a module's multiple file output
                                 is placed.
  --power-assertions / --no-power-assertions
                                 Enable power assertions for detailed assertion
                                 failure messages.
  -h, --help                     Show this message and exit

Arguments:
  <modules>  Module paths or URIs to evaluate.

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_TEST: &str = "\
Usage: pkl test [<options>] [<modules>]...

  Run tests within the given module(s)

Options:
  --allowed-modules=<pattern1,pattern2>
                                  URI patterns that determine which modules can
                                  be loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                                  URI patterns that determine which external
                                  resources can be read.
  --root-dir=<path>               Restricts access to file-based modules and
                                  resources to those located under the root
                                  directory.
  --cache-dir=<path>              The cache directory for storing packages.
  -w, --working-dir=<path>        Base path that relative module paths are
                                  resolved against.
  -p, --property=<name=value>     External property to set (repeatable).
  --color=<when>                  Whether to format messages in ANSI color.
                                  Possible values of <when> are 'never', 'auto',
                                  and 'always'.
  --no-cache                      Disable caching of packages
  -f, --format=<text>             Output format to generate. <json, jsonnet,
                                  pcf, properties, plist, textproto, xml, yaml,
                                  pkl-binary>
  -e, --env-var=<name=value>      Environment variable to set (repeatable).
  --module-path=<path1:path2>     Directories, ZIP archives, or JAR archives to
                                  search when resolving modulepath: URIs.
  --settings=<value>              Pkl settings module to use.
  -t, --timeout=<number>          Duration, in seconds, after which evaluation
                                  of a source module will be timed out.
  --ca-certificates=<path>        Only trust CA certificates from the provided
                                  file(s).
  --http-proxy=<address>          Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                                  Hostnames that should not be connected to via
                                  a proxy.
  --http-rewrite=<from=to>        URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                                  HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                                  External reader registrations for module URI
                                  schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                                  External reader registrations for resource URI
                                  schemes
  --trace-mode=<style>            Specifies how calls to trace() are formatted.
                                  Possible values of <style> are 'compact' and
                                  'pretty'.
  --project-dir=<path>            The project directory to use for this command.
                                  By default, searches up from the working
                                  directory for a PklProject file.
  --omit-project-settings         Ignores evaluator settings set in the
                                  PklProject file.
  --no-project                    Disables loading settings and dependencies
                                  from the PklProject file.
  --junit-reports=<dir>           Directory where to store JUnit reports.
  --junit-aggregate-reports       Aggregate JUnit reports into a single file.
  --junit-aggregate-suite-name=<name>
                                  The name of the root JUnit test suite.
  --overwrite                     Force generation of expected examples.
  --test-reporter=(spec|minimal)  Which test reporter to use for CLI output.
  --power-assertions / --no-power-assertions
                                  Enable power assertions for detailed assertion
                                  failure messages.
  -h, --help                      Show this message and exit

Arguments:
  <modules>  Module paths or URIs to evaluate.

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_REPL: &str = "\
Usage: pkl repl [<options>]

  Start a REPL session

Options:
  --allowed-modules=<pattern1,pattern2>
                               URI patterns that determine which modules can be
                               loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                               URI patterns that determine which external
                               resources can be read.
  --root-dir=<path>            Restricts access to file-based modules and
                               resources to those located under the root
                               directory.
  --cache-dir=<path>           The cache directory for storing packages.
  -w, --working-dir=<path>     Base path that relative module paths are resolved
                               against.
  -p, --property=<name=value>  External property to set (repeatable).
  --color=<when>               Whether to format messages in ANSI color.
                               Possible values of <when> are 'never', 'auto',
                               and 'always'.
  --no-cache                   Disable caching of packages
  -f, --format=<text>          Output format to generate. <json, jsonnet, pcf,
                               properties, plist, textproto, xml, yaml,
                               pkl-binary>
  -e, --env-var=<name=value>   Environment variable to set (repeatable).
  --module-path=<path1:path2>  Directories, ZIP archives, or JAR archives to
                               search when resolving modulepath: URIs.
  --settings=<value>           Pkl settings module to use.
  -t, --timeout=<number>       Duration, in seconds, after which evaluation of a
                               source module will be timed out.
  --ca-certificates=<path>     Only trust CA certificates from the provided
                               file(s).
  --http-proxy=<address>       Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                               Hostnames that should not be connected to via a
                               proxy.
  --http-rewrite=<from=to>     URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                               HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for module URI
                               schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for resource URI
                               schemes
  --trace-mode=<style>         Specifies how calls to trace() are formatted.
                               Possible values of <style> are 'compact' and
                               'pretty'.
  --project-dir=<path>         The project directory to use for this command. By
                               default, searches up from the working directory
                               for a PklProject file.
  --omit-project-settings      Ignores evaluator settings set in the PklProject
                               file.
  --no-project                 Disables loading settings and dependencies from
                               the PklProject file.
  -h, --help                   Show this message and exit

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_SERVER: &str = "\
Usage: pkl server [<options>]

  Run as a server that communicates over standard input/output

Options:
  -h, --help  Show this message and exit

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_FORMAT: &str = "\
Usage: pkl format [<options>] [<paths>]...

  Format or check formatting of Pkl files.

  Examples:

  ╭───────────────────────────────────────────────────────────────────╮
  │# Overwrite all Pkl files inside `my/folder/`, recursively.        │
  │$ pkl format -w my/folder/                                         │
  │                                                                   │
  │# Check formatting of all files, printing filenames with formatting│
  │violations to stdout.                                              │
  │# Exit with exit code `11` if formatting violations were found.    │
  │$ pkl format --diff-name-only my/folder/                           │
  │                                                                   │
  │# Format Pkl code from stdin.                                      │
  │$ echo \"foo = 1\" | pkl format -                                    │
  ╰───────────────────────────────────────────────────────────────────╯

Options:
  --grammar-version=(1|2)  The grammar compatibility version to use.
                           1: 0.25 - 0.29
                           2: 0.30+ (default)
  -w, --write              Format files in place, overwriting them. Implies
                           ---diff-name-only.
  --diff-name-only         Write the path of files with formatting violations to
                           stdout.
  -s, --silent             Don't write to stdout or stderr. Mutually exclusive
                           with --diff-name-only.
  -h, --help               Show this message and exit

Arguments:
  <paths>  Files or directory to check. Use - to read from stdin.

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_RUN: &str = "\
Usage: pkl run [<options>] [<module>] [<args>]...

  Run a Pkl pkl:Command CLI tool

Options:
  --allowed-modules=<pattern1,pattern2>
                               URI patterns that determine which modules can be
                               loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                               URI patterns that determine which external
                               resources can be read.
  --root-dir=<path>            Restricts access to file-based modules and
                               resources to those located under the root
                               directory.
  --cache-dir=<path>           The cache directory for storing packages.
  -w, --working-dir=<path>     Base path that relative module paths are resolved
                               against.
  -p, --property=<name=value>  External property to set (repeatable).
  --color=<when>               Whether to format messages in ANSI color.
                               Possible values of <when> are 'never', 'auto',
                               and 'always'.
  --no-cache                   Disable caching of packages
  -f, --format=<text>          Output format to generate. <json, jsonnet, pcf,
                               properties, plist, textproto, xml, yaml,
                               pkl-binary>
  -e, --env-var=<name=value>   Environment variable to set (repeatable).
  --module-path=<path1:path2>  Directories, ZIP archives, or JAR archives to
                               search when resolving modulepath: URIs.
  --settings=<value>           Pkl settings module to use.
  -t, --timeout=<number>       Duration, in seconds, after which evaluation of a
                               source module will be timed out.
  --ca-certificates=<path>     Only trust CA certificates from the provided
                               file(s).
  --http-proxy=<address>       Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                               Hostnames that should not be connected to via a
                               proxy.
  --http-rewrite=<from=to>     URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                               HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for module URI
                               schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for resource URI
                               schemes
  --trace-mode=<style>         Specifies how calls to trace() are formatted.
                               Possible values of <style> are 'compact' and
                               'pretty'.
  -h, --help                   Show this message and exit
  --project-dir=<path>         The project directory to use for this command. By
                               default, searches up from the working directory
                               for a PklProject file.
  --omit-project-settings      Ignores evaluator settings set in the PklProject
                               file.
  --no-project                 Disables loading settings and dependencies from
                               the PklProject file.

Arguments:
  <module>  Root pkl:Command module to invoke.
  <args>

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_SHELL_COMPLETION: &str = "\
Usage: pkl shell-completion [<options>] <shell>

  Generate a completion script for the given shell

Options:
  -h, --help  Show this message and exit

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_PROJECT: &str = "\
Usage: pkl project [<options>] <command> [<args>]...

  Run commands related to projects

Options:
  -h, --help  Show this message and exit

Commands:
  resolve  Resolve dependencies for project(s)
  package  Verify package(s), and prepare package artifacts to be published.

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_PROJECT_RESOLVE: &str = "\
Usage: pkl project resolve [<options>] [<dir>]...


  Resolve dependencies for project(s)

  This command takes the dependencies of PklProjects, and writes the resolved
  versions to PklProject.deps.json files.

  Examples:

  ╭───────────────────────────────────────────────────────────────────────────╮
  │  # Search the current working directory for a project, and resolve its    │
  │dependencies.                                                              │
  │  $ pkl project resolve                                                    │
  │                                                                           │
  │  # Resolve dependencies for all projects within the `packages/` directory.│
  │  $ pkl project resolve packages/*/                                        │
  ╰───────────────────────────────────────────────────────────────────────────╯

Options:
  --allowed-modules=<pattern1,pattern2>
                               URI patterns that determine which modules can be
                               loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                               URI patterns that determine which external
                               resources can be read.
  --root-dir=<path>            Restricts access to file-based modules and
                               resources to those located under the root
                               directory.
  --cache-dir=<path>           The cache directory for storing packages.
  -w, --working-dir=<path>     Base path that relative module paths are resolved
                               against.
  -p, --property=<name=value>  External property to set (repeatable).
  --color=<when>               Whether to format messages in ANSI color.
                               Possible values of <when> are 'never', 'auto',
                               and 'always'.
  --no-cache                   Disable caching of packages
  -f, --format=<text>          Output format to generate. <json, jsonnet, pcf,
                               properties, plist, textproto, xml, yaml,
                               pkl-binary>
  -e, --env-var=<name=value>   Environment variable to set (repeatable).
  --module-path=<path1:path2>  Directories, ZIP archives, or JAR archives to
                               search when resolving modulepath: URIs.
  --settings=<value>           Pkl settings module to use.
  -t, --timeout=<number>       Duration, in seconds, after which evaluation of a
                               source module will be timed out.
  --ca-certificates=<path>     Only trust CA certificates from the provided
                               file(s).
  --http-proxy=<address>       Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                               Hostnames that should not be connected to via a
                               proxy.
  --http-rewrite=<from=to>     URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                               HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for module URI
                               schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for resource URI
                               schemes
  --trace-mode=<style>         Specifies how calls to trace() are formatted.
                               Possible values of <style> are 'compact' and
                               'pretty'.
  -h, --help                   Show this message and exit

Arguments:
  <dir>  The project directories to resolve dependencies for

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_PROJECT_PACKAGE: &str = "\
Usage: pkl project package [<options>] [<dir>]...

  Verify package(s), and prepare package artifacts to be published.

  This command runs a project's api tests, as defined by apiTests in PklProject.
  Additionally, it verifies that all imports resolve to paths that are local to
  the project.

  Finally, this command writes the following artifacts into the output directory
  specified by the output path.

   • name@version - dependency metadata
   • name@version.sha256 - dependency metadata's SHA-256 checksum
   • name@version.zip - package archive
   • name@version.zip.sha256 - package archive's SHA-256 checksum

  The output path option accepts the following placeholders:

   • %{name}: The display name of the package
   • %{version}: The version of the package

  If a project has local project dependencies, the depended upon project
  directories must also be included as arguments to this command.

  Examples:

  ╭─────────────────────────────────────────────────────────────────────╮
  │# Search the current working directory for a project, and package it.│
  │$ pkl project package                                                │
  │                                                                     │
  │# Package all projects within the `packages/` directory.             │
  │$ pkl project package packages/*/                                    │
  ╰─────────────────────────────────────────────────────────────────────╯

Options:
  --allowed-modules=<pattern1,pattern2>
                                  URI patterns that determine which modules can
                                  be loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                                  URI patterns that determine which external
                                  resources can be read.
  --root-dir=<path>               Restricts access to file-based modules and
                                  resources to those located under the root
                                  directory.
  --cache-dir=<path>              The cache directory for storing packages.
  -w, --working-dir=<path>        Base path that relative module paths are
                                  resolved against.
  -p, --property=<name=value>     External property to set (repeatable).
  --color=<when>                  Whether to format messages in ANSI color.
                                  Possible values of <when> are 'never', 'auto',
                                  and 'always'.
  --no-cache                      Disable caching of packages
  -f, --format=<text>             Output format to generate. <json, jsonnet,
                                  pcf, properties, plist, textproto, xml, yaml,
                                  pkl-binary>
  -e, --env-var=<name=value>      Environment variable to set (repeatable).
  --module-path=<path1:path2>     Directories, ZIP archives, or JAR archives to
                                  search when resolving modulepath: URIs.
  --settings=<value>              Pkl settings module to use.
  -t, --timeout=<number>          Duration, in seconds, after which evaluation
                                  of a source module will be timed out.
  --ca-certificates=<path>        Only trust CA certificates from the provided
                                  file(s).
  --http-proxy=<address>          Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                                  Hostnames that should not be connected to via
                                  a proxy.
  --http-rewrite=<from=to>        URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                                  HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                                  External reader registrations for module URI
                                  schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                                  External reader registrations for resource URI
                                  schemes
  --trace-mode=<style>            Specifies how calls to trace() are formatted.
                                  Possible values of <style> are 'compact' and
                                  'pretty'.
  --junit-reports=<dir>           Directory where to store JUnit reports.
  --junit-aggregate-reports       Aggregate JUnit reports into a single file.
  --junit-aggregate-suite-name=<name>
                                  The name of the root JUnit test suite.
  --overwrite                     Force generation of expected examples.
  --test-reporter=(spec|minimal)  Which test reporter to use for CLI output.
  --output-path=<path>            The directory to write artifacts to
  --skip-publish-check            Skip checking if a package has already been
                                  published with different contents
  -h, --help                      Show this message and exit

Arguments:
  <dir>  The project directories to package

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_DOWNLOAD_PACKAGE: &str = "\
Usage: pkl download-package [<options>] <package>...

  Download package(s)

  This command downloads the specified packages to the cache directory. If the
  package already exists in the cache directory, this command is a no-op.

  Examples:

  ╭───────────────────────────────────────────────────────────╮
  │# Download two packages                                    │
  │$ pkl download-package package://example.com/package1@1.0.0│
  │package://example.com/package2@1.0.0                       │
  ╰───────────────────────────────────────────────────────────╯

Options:
  --allowed-modules=<pattern1,pattern2>
                               URI patterns that determine which modules can be
                               loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                               URI patterns that determine which external
                               resources can be read.
  --root-dir=<path>            Restricts access to file-based modules and
                               resources to those located under the root
                               directory.
  --cache-dir=<path>           The cache directory for storing packages.
  -w, --working-dir=<path>     Base path that relative module paths are resolved
                               against.
  -p, --property=<name=value>  External property to set (repeatable).
  --color=<when>               Whether to format messages in ANSI color.
                               Possible values of <when> are 'never', 'auto',
                               and 'always'.
  --no-cache                   Disable caching of packages
  -f, --format=<text>          Output format to generate. <json, jsonnet, pcf,
                               properties, plist, textproto, xml, yaml,
                               pkl-binary>
  -e, --env-var=<name=value>   Environment variable to set (repeatable).
  --module-path=<path1:path2>  Directories, ZIP archives, or JAR archives to
                               search when resolving modulepath: URIs.
  --settings=<value>           Pkl settings module to use.
  -t, --timeout=<number>       Duration, in seconds, after which evaluation of a
                               source module will be timed out.
  --ca-certificates=<path>     Only trust CA certificates from the provided
                               file(s).
  --http-proxy=<address>       Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                               Hostnames that should not be connected to via a
                               proxy.
  --http-rewrite=<from=to>     URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                               HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for module URI
                               schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for resource URI
                               schemes
  --trace-mode=<style>         Specifies how calls to trace() are formatted.
                               Possible values of <style> are 'compact' and
                               'pretty'.
  --project-dir=<path>         The project directory to use for this command. By
                               default, searches up from the working directory
                               for a PklProject file.
  --omit-project-settings      Ignores evaluator settings set in the PklProject
                               file.
  --no-project                 Disables loading settings and dependencies from
                               the PklProject file.
  --no-transitive              Skip downloading transitive dependencies of a
                               package
  -h, --help                   Show this message and exit

Arguments:
  <package>  The package URIs to download

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_ANALYZE: &str = "\
Usage: pkl analyze [<options>] <command> [<args>]...

  Commands related to static analysis

Options:
  -h, --help  Show this message and exit

Commands:
  imports  Prints the graph of modules imported by the input module(s).

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const HELP_ANALYZE_IMPORTS: &str = "\
Usage: pkl analyze imports [<options>] <modules>...

  Prints the graph of modules imported by the input module(s).

Options:
  --allowed-modules=<pattern1,pattern2>
                               URI patterns that determine which modules can be
                               loaded and evaluated.
  --allowed-resources=<pattern1,pattern2>
                               URI patterns that determine which external
                               resources can be read.
  --root-dir=<path>            Restricts access to file-based modules and
                               resources to those located under the root
                               directory.
  --cache-dir=<path>           The cache directory for storing packages.
  -w, --working-dir=<path>     Base path that relative module paths are resolved
                               against.
  -p, --property=<name=value>  External property to set (repeatable).
  --color=<when>               Whether to format messages in ANSI color.
                               Possible values of <when> are 'never', 'auto',
                               and 'always'.
  --no-cache                   Disable caching of packages
  -f, --format=<text>          Output format to generate. <json, jsonnet, pcf,
                               properties, plist, textproto, xml, yaml,
                               pkl-binary>
  -e, --env-var=<name=value>   Environment variable to set (repeatable).
  --module-path=<path1:path2>  Directories, ZIP archives, or JAR archives to
                               search when resolving modulepath: URIs.
  --settings=<value>           Pkl settings module to use.
  -t, --timeout=<number>       Duration, in seconds, after which evaluation of a
                               source module will be timed out.
  --ca-certificates=<path>     Only trust CA certificates from the provided
                               file(s).
  --http-proxy=<address>       Proxy to use for HTTP(S) connections.
  --http-no-proxy=<pattern1,pattern>
                               Hostnames that should not be connected to via a
                               proxy.
  --http-rewrite=<from=to>     URL prefixes that should be rewritten.
  --http-header=<url-pattern>=<header name>:<header value>
                               HTTP header to add to the request.
  --external-module-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for module URI
                               schemes
  --external-resource-reader=<scheme>='<executable> [<arguments>]'>
                               External reader registrations for resource URI
                               schemes
  --trace-mode=<style>         Specifies how calls to trace() are formatted.
                               Possible values of <style> are 'compact' and
                               'pretty'.
  --project-dir=<path>         The project directory to use for this command. By
                               default, searches up from the working directory
                               for a PklProject file.
  --omit-project-settings      Ignores evaluator settings set in the PklProject
                               file.
  --no-project                 Disables loading settings and dependencies from
                               the PklProject file.
  -o, --output-path=<path>     File path where the output file is placed.
  -h, --help                   Show this message and exit

Arguments:
  <modules>  Module paths or URIs to evaluate.

For more information, visit
https://pkl-lang.org/main/0.32.0/pkl-cli/index.html#usage
";

pub const COMPLETION_BASH: &str = include_str!("../assets/completion_bash.sh");
pub const COMPLETION_ZSH: &str = include_str!("../assets/completion_zsh.sh");
pub const COMPLETION_FISH: &str = include_str!("../assets/completion_fish.fish");

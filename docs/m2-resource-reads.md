# Resource-read language development contract

The shared runtime is extending `read` and `read?` against the pinned Pkl 0.32.0
reference. This contract covers language parsing, operand evaluation, selected
URI rules, the default scalar development profile and complete diagnostics.
Module loading, resource readers and configurable evaluator products remain
separate implementation obligations.

## Keywords and operands

Unquoted `read` and `read?` are language expressions with one parenthesized
operand. The question mark is part of the adjacent keyword; whitespace or a
comment before the opening parenthesis is allowed. The operand is evaluated
once, before URI parsing and resource policy processing. A non-string operand
produces the operator's type error without forcing an object's members.

Quoted names such as `` `read` `` and `` `read?` `` remain ordinary method names.
Declaring such a method does not shadow an unquoted read keyword. Missing quoted
methods use the common method-lookup diagnostic, including the full call source
section and applicable suggestions. Read operands do not inherit contextual
`new {}` inference from the surrounding property type.

`read?` returns `null` for an absent resource. It propagates invalid URI,
authorization, operand and reader failures. It is not a general error-catching
operator. The current default scalar profile implements absence for its
explicitly empty environment-variable and external-property maps.

## Shared default profile

Native, Wasm and N-API scalar development adapters use the same runtime poll
loop. A pending resource request is completed through the normal VM resume path,
so errors retain authored read frames, callers, forcing order and failed-attempt
cleanup. Retrying a failed member preserves other successful cached results.

The URI path keeps authored text separate from resolved and normalized text.
Validation precedes the applicable authorization check. Hierarchical literal dot
segments are normalized; opaque URIs and encoded dot segments retain their
distinct behavior. Relative paths use declaration-module source metadata, with
the primary source as the current single-module fallback.

The pinned default policy recognizes case-sensitive prefixes for `prop:`,
`env:`, `file:`, `modulepath:`, `package:`, `projectpackage:` and `https:`.
Authorization does not imply that a reader is implemented. The development
profile currently supplies no permitted file, network or package I/O. Environment
and external-property reads use explicitly empty configured maps; they do not
read the surrounding process or JVM defaults.

Policy-denial diagnostics show the resolved, normalized URI. Missing-resource
diagnostics retain the authored URI. Registered HTTP readers may encounter
request-construction failures before checking authorization; unsupported request
forms remain explicit gaps rather than invented authorization errors.

Explicit custom Host readers retain their callback and completion contract.
The scalar default policy is not silently applied to every custom reader.
Complete nullable-absence transport through public Host APIs, resource caching
and product evaluator settings remain open.

## Evidence boundaries

The original fixed-URI Java matrix includes controlled filesystem inputs and
lossless UTF-16 captures. All original contracts remain in that comparison,
including unsupported file reads and lone-surrogate URI diagnostics. Standard
source-path corpus cases preserve authored source bytes but acquire fresh Java
diagnostics at their own recorded source URI. These two boundaries are not
interchangeable.

Actual Node and Chromium scalar comparisons use the fixed portable source URI
and per-case power-assertion settings. Java internal errors retain their original
payloads and failure classification. Complete-byte equality with an internal
error does not establish a successful language result.

For cases with Java UTF-16 captures, the portable string API comparison requires
identical UTF-16 code units. It also compares Node's UTF-8 encoding of the actual
string with Node's encoding of the independently captured Java units. Original
Java `String.getBytes(UTF_8)` bytes remain separately retained and compared:
Java and Node use different replacements for an unmatched surrogate. Reports
identify that serialization difference without treating it as string equality
or claiming cross-encoder byte equality. Cases without lossless unit captures
retain their existing raw UTF-8 comparison contract. Native source-path probes
continue to compare their original stdout and stderr bytes directly.

## Remaining scope

Complete Java URI syntax and authority rules, URI strings containing unmatched
UTF-16 surrogates, permitted I/O readers, configurable policy, dependency and
triple-dot resolution, resource caches, globbed reads and module loading remain
open. The current UTF-8 request/error boundary does not establish lossless URI
diagnostic transport. Selected resource and method controls do not complete the
M2 language gate, product APIs, release-platform coverage or performance targets.

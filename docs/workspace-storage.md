# Workspace test storage on macOS

`tools/workspace_storage.py` is an opt-in `rustc` wrapper for workspace
verification on native macOS arm64 with an APFS target directory. It applies
transparent filesystem compression to linked Mach-O executables before Cargo
starts them. The wrapper does not select tests or change Cargo's test command.
The resulting executables have the same logical bytes; compression is not a
performance or language-compatibility claim.

Use a new, isolated Cargo target directory and a separate new receipt
directory. Run the following from the repository root, with the pinned Rust
toolchain installed. Choose unused names for both directories:

```sh
mkdir -p target
mkdir target/workspace-storage-build target/workspace-storage-receipts
export CARGO_TARGET_DIR="$PWD/target/workspace-storage-build"
export PKL_STORAGE_TARGET="$CARGO_TARGET_DIR"
export PKL_STORAGE_RECEIPTS="$PWD/target/workspace-storage-receipts"
export RUSTC_WRAPPER="$PWD/tools/workspace_storage.py"
cargo test --workspace --locked
```

The target and receipt paths must be absolute, separate, existing directories
without symlink components. The wrapper rejects linked artifacts outside the
specified target directory. Use the same isolated target for the full run;
do not point it at a cache or artifacts that must remain unchanged. Unset
`RUSTC_WRAPPER`, `PKL_STORAGE_TARGET`, `PKL_STORAGE_RECEIPTS`, and
`CARGO_TARGET_DIR` after the experiment if they should not apply to later
commands.

The wrapper requires a minimum of **1,024 MiB** free by default, plus enough
space for the full logical size of an executable while its temporary copy is
created. It checks the floor during the copy and before replacement.
`PKL_STORAGE_MIN_FREE_MIB` can set an integer from 512 through 65,536 MiB;
the selected value is recorded in each receipt. Keep the default for ordinary
use. A lower floor is a separate, explicitly chosen resource experiment and
does not change the outcome or conditions of an earlier run. The optional
`PKL_STORAGE_COPY_SECONDS` limits each copy to 1 through 1,800 seconds and
defaults to 120 seconds.

For each compiler invocation, the wrapper forwards the compiler's stdout and
stderr and writes raw copies and a JSON receipt in the receipt directory. It
uses rustc's JSON link-artifact notifications to identify products. A
successful explicit test or binary link without a notification fails closed;
compiler probes and metadata-only invocations do not require one. Libraries
and other non-executable link products are not compressed.

Before replacing an executable, the wrapper checks that it is a regular,
single-link Mach-O executable, verifies its code signature, and creates a
unique same-directory compressed copy using `ditto`. It compares the logical
bytes and SHA-256, mode, owner, group, ACL, and non-compression extended
attributes, then verifies the copy's signature. It permits only the compression
flag transition before atomically replacing the original. File inode and
timestamps can change. Dependency files and Cargo's test selections are not
rewritten. The target directory must be trusted and isolated: path and identity
checks do not lock out concurrent writers.

The wrapper enforces its copy deadline and disk floor; it does **not** impose a
standalone RSS or total workspace timeout. The operator must supervise memory,
elapsed time, free space, and log growth, and retain complete Cargo test output
and terminal status outside the wrapper receipts. Treat a stopped or failed
command as such; per-invocation compiler receipts do not substitute for a
completed workspace test run.

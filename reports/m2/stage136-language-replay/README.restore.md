# Restoring the Stage136 evidence files

The checkpoint descriptor records the SHA-256 of the complete restoration index. Keep `restore.py` and `restore_reader.py` together, and supply every archive part and that index hash. The output directory must not exist, its parent must already exist, and it must be outside the repository. For example, replace the hash and paths below with the published values:

```sh
python3 -B restore.py --index-sha256 INDEX_SHA256 \
  --part-sha256 PART_001_SHA256 --part-sha256 PART_002_SHA256 \
  /private/tmp/stage136-restored \
  stage136-evidence-001.tar.gz stage136-evidence-002.tar.gz
```

List all published parts and pass one `--part-sha256` for each part in the same order. The utility checks those hashes before creating output. If part hashes are omitted, it prints the actual part hashes; compare them with a trusted descriptor to establish archive-byte provenance. Parts may be supplied in any order when their expected hashes follow the same order. The utility checks each gzip and tar stream, reconstructs and verifies the split index against the supplied hash, checks chunk identities and the full path/mode roster, and rejects symlinks in archive and destination paths before creating the output. It then restores regular-file bytes and their recorded ordinary permission bits. Publication rejects files with setuid, setgid or sticky bits rather than silently changing those modes. It does not extract archive member paths directly. The reconstructed index is limited to 512 MiB; the publisher applies the same bound. A failed restore can leave a partial newly created output directory; inspect or remove that directory before retrying with a new destination.

The index records regular files. Empty directories and directory permissions are not reconstructed. The restored evidence is not a complete compiler, runtime, operating-system or external-service environment, and archive integrity is not a claim of full language compatibility. The descriptor and archive part hashes must be obtained from a trusted checkpoint source; the utility does not authenticate that source.

The utility has been exercised with synthetic files on local macOS. It requires POSIX directory-relative operations and `O_DIRECTORY`, `O_NOFOLLOW`, and `O_NONBLOCK`; unsupported platforms fail closed. Windows support has not been established.

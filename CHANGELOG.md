# git-dit ChangeLog

## v0.2.1 (2017-07-16)

Changes:
 * Add WORKFLOWS section to the man page
 * Clarify implementation of metadata "tags" in the documentation
 * Clarify membership of initial issue messages to branches in man page
 * Fix punktuation issues in the man page

Bug-fixes:
 * Fix bug in `Trailers` iterator which caused emission of trailers even if they
   are embedded in a block of text.


## v0.2 (2017-06-22)

Changes:
 * Rewrite in Rust (single binary)
 * Drop "prepare-metadata" script
 * "push" and "fetch" now only support dummy auth and ssh-agent
 * Updated README
 * Updated man page

Added features:
 * Documentation of data model, semantics and use-cases
 * Library crate "libgitdit"
 * Long options

Bug-fixes:
 * Read editor from git config, fall back to default
   (We previously relied on the `EDITOR` variable.)


## v0.1 (2017-02-01)

Initial prototype, written in bash.


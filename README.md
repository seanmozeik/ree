# ree

`ree` repairs a terminal after a program leaves it in raw mode or changes the
emulator state. Type `ree` blind and press Enter when input, echo, the cursor,
or the screen is broken.

The command repairs the kernel TTY state, reads the compiled terminfo entry,
and writes the same reset capabilities used by `tput reset`. It has no ncurses
dependency and does not use the historical one-second hardware-settling sleep.

`ree` is a Rust rewrite of Guillermo Rauch's
[`rst`](https://github.com/rauchg/rst). The terminal behavior and Apache-2.0
license come from that project.

## What it repairs

### Kernel TTY state

`ree` restores canonical input, echo, signals, CR/NL mapping, and output
processing. It repairs a control character only when that character is
disabled, so a valid customization such as an erase key set to `^H` remains in
place.

The update uses `TCSAFLUSH` to discard stale typeahead from a crashed raw-mode
program. It also resumes a stopped output queue before reading or changing the
termios state, which recovers a terminal stuck after `^S` or `TCOOFF`.

The command checks the terminal foreground process group before it changes or
writes anything. A background job exits without a diagnostic because writing
that diagnostic could itself send `SIGTTOU`.

### Terminal emulator state

`ree` reads the standard and extended compiled terminfo formats directly. It
emits these capabilities in ncurses order:

1. `rs1`, with `is1` as its fallback
2. `rs2`, with `is2` as its fallback
3. `clear_margins`
4. `rs3`, with `is3` as its fallback

Known VT-compatible terminals first receive a cleanup sequence. It ends a
stray terminal string, disables synchronized output, mouse and focus reports,
bracketed paste, terminal reports, and Kitty paste events. It also clears the
Kitty keyboard stack and disables xterm `modifyOtherKeys`. If no terminfo entry
exists, the terminal receives a fixed VT reset sequence. This fallback is
useful when a newer local terminal connects to an older host over SSH.

`ree` looks for a writable TTY on stderr, stdout, stdin, and `/dev/tty`, in that
order. A TTY found on read-only stdin is reopened read-write.

### Ghostty and shells

Ghostty uses `TERM=xterm-ghostty` and supplies its compiled terminfo entry
through `TERMINFO`. `ree` searches that location first. Its cleanup covers
Ghostty's synchronized output, mouse encodings, focus reports, color-scheme and
visibility reports, in-band size reports, Kitty paste events, Kitty keyboard
stack, and `modifyOtherKeys`. Ghostty's `rs1` then sends RIS, which resets the
screen, alternate screen, keyboard state, terminal modes, and progress state.

The command does not depend on a specific shell. Nushell uses Reedline, which
can enable bracketed paste and push Kitty keyboard flags while it reads input.
`ree` disables both states. Bash, Elvish, Fish, Nushell, and Zsh can all run the
same binary. When a remote host does not have the `xterm-ghostty` entry, the VT
fallback still sends RIS.

## Examples

| Break command | Result | Repaired |
| --- | --- | --- |
| `cat /dev/urandom` | Random escape sequences change terminal state | Yes |
| `printf '\e[?1049h'` | Alternate screen hides scrollback | Yes |
| `printf '\e[?25l'` | Cursor disappears | Yes |
| `printf '\e[8m'` | Text becomes concealed | Yes |
| `printf '\e(0'` | DEC line-drawing characters replace text | Yes |
| `printf '\e[?1003h\e[?1006h'` | Mouse events appear as input | Yes |
| `stty raw` | Line editing, echo, and signals stop | Yes |
| `stty -opost -onlcr` | Output starts to stair-step | Yes |

## CLI

```text
ree
ree --help
ree --version
```

The CLI is declared with
[`usage-rs`](https://github.com/jdx/usage). The same typed declaration handles
argument parsing, help, version output, diagnostics, and the portable Usage
spec endpoint:

```text
ree __usage_spec__
```

The endpoint prints KDL that `usage` can convert to documentation, man pages,
completions, or JSON.

## Install

The crates.io package is named `ree-cli` because another project owns the
`ree` crate name. The npm package uses Sean's scope. Both packages install a
command named `ree`.

```text
cargo install ree-cli
bun add --global @seanmozeik/ree
```

The npm release contains these native packages:

| System | CPU | Rust target |
| --- | --- | --- |
| macOS | arm64 | `aarch64-apple-darwin` |
| macOS | x86-64 | `x86_64-apple-darwin` |
| Linux, glibc | arm64 | `aarch64-unknown-linux-gnu` |
| Linux, glibc | x86-64 | `x86_64-unknown-linux-gnu` |

Linux binaries use a glibc 2.17 baseline.

## Build

`ree` requires Rust 1.95 or newer. The repository selects the stable toolchain
and uses Rust 2024 edition.

```text
just build-release
just build-all
just install
```

The release profile optimizes for size and uses fat LTO, one codegen unit,
abort-on-panic, and symbol stripping. It links the standard system libraries
and does not link ncurses. `just size-check` enforces a 600,000-byte host
budget. `just size-check-all` applies the same limit to all four release
binaries. `just glibc-check` enforces the Linux ABI baseline.

## Package and publish

The Cargo version is the one version source for crates.io and all five npm
packages. Packaging reads Cargo metadata and uses binaries from `target/`.

```text
just pack
just publish-cargo-dry-run
just publish-npm-dry-run
just verify-release
```

The npm publisher sends the four platform packages before the root package.
The dry-run recipes do not change either registry. The real publish recipes
are `just publish-cargo` and `just publish-npm`.

## Test

```text
just check
just test
just test-doc
just pty
just size-check
```

`just verify` runs the local gates. The recipes use strict shell settings,
grouped help, locked Cargo commands, optional `rtk`, a `nextest` fallback, and
a 350-line source-file budget. `just verify-release` also checks every release
artifact and both registry packages.

The PTY test zeros the input, output, and local termios flags, disables every
control character, and checks the repaired flag groups. It checks
`xterm-ghostty`, a missing-terminfo fallback, a background process group, a
process without terminal ownership, the complete VT cleanup, and finite child
timeouts. The unit suite covers both terminfo headers, search precedence,
malformed offsets, unterminated strings, size limits, padding grammar, terminal
family boundaries, every termios flag group, and 256 generated parser inputs
per run.

## Differences from ncurses

`ree` follows the reset path used by ncurses `reset` and `tput reset`, with
these deliberate changes:

- It omits the historical settling sleep.
- It does not reprogram hardware tab stops.
- It does not read `reset_file` or `init_file` capabilities.
- It sends `clear_margins` when present and omits ncurses' alternate margin
  fallback.
- It adds the VT cleanup prelude for known terminal emulators.
- It uses a fixed VT sequence when the terminfo entry is missing.
- It removes terminfo delay markers instead of sleeping or writing pad bytes.

These choices target terminal emulators and pseudo-terminals. Use the system
`reset` command for a physical or serial terminal that depends on hardware
delays, tab-stop programming, or alternate margin handling.

## Prior art

- [rauchg/rst](https://github.com/rauchg/rst) supplies the direct design and
  compatibility target for this Rust implementation.
- [BusyBox reset](https://git.busybox.net/busybox/tree/console-tools/reset.c)
  restores a small fixed set of terminal behaviors without a settling sleep.
- [Toybox reset](https://github.com/landley/toybox/blob/master/toys/other/reset.c)
  combines terminal mode repair with fixed escape sequences.
- ncurses routes both `reset` and `tput reset` through its shared reset code.

## License

Apache-2.0. See [LICENSE](LICENSE).

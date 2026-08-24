# ree

`ree` restores a terminal after a program leaves the terminal driver or
terminal emulator in an unusable state. If the terminal does not show input,
type `ree` and press Enter. If raw mode prevents Enter from working, press
Ctrl-J.

`ree` is a Rust fork of Guillermo Rauch's
[`rst`](https://github.com/rauchg/rst). The fork keeps the direct terminfo
design and Apache-2.0 license from `rst`. It adds stricter terminal ownership
checks, validated terminfo handling, extended Ghostty and Nushell recovery, a
[`usage-rs`](https://github.com/jdx/usage) command-line interface, and native
packages for Cargo and npm.

## Install a release

Each package installs a command named `ree`.

```text
cargo install ree-cli
npm install --global @seanmozeik/ree
bun add --global @seanmozeik/ree
```

The npm package selects one of these native packages:

| Operating system | CPU | Rust target |
| --- | --- | --- |
| macOS | arm64 | `aarch64-apple-darwin` |
| macOS | x86-64 | `x86_64-apple-darwin` |
| Linux with glibc | arm64 | `aarch64-unknown-linux-gnu` |
| Linux with glibc | x86-64 | `x86_64-unknown-linux-gnu` |

The Linux binaries require glibc 2.17 or a later compatible version.

To install from source:

```text
git clone https://github.com/seanmozeik/ree.git
cd ree
just install
```

## Restore a terminal

Run `ree` without arguments:

```text
ree
```

The command performs these operations:

1. Find a terminal on standard error, standard output, standard input, or
   `/dev/tty`.
2. Confirm that `ree` is in the foreground process group.
3. Resume a stopped output queue.
4. Repair the terminal driver state.
5. load the compiled terminfo entry for `TERM`.
6. disable terminal emulator modes that can affect the shell.
7. write the terminfo reset capabilities.

`ree` exits without writing to the terminal when another process group owns
the terminal. This behavior prevents a background job from changing the active
terminal or receiving `SIGTTOU`.

Use these commands to inspect the command-line interface:

```text
ree --help
ree --version
ree __usage_spec__
```

`ree __usage_spec__` writes the Usage KDL specification. The `usage` tool can
convert this specification to completion scripts, man pages, documentation, or
JSON.

## Terminal driver recovery

`ree` repairs these terminal driver settings:

- canonical input
- input echo
- signal processing
- carriage-return and newline mapping
- output processing
- disabled control characters

The command changes a control character only when the character is disabled.
It preserves a valid customization, such as an erase key set to `Ctrl-H`.

`ree` applies the repaired state with `TCSAFLUSH`. This operation discards
unread input that a failed raw-mode program can leave in the input queue. The
command also resumes output that `Ctrl-S` or `TCOOFF` stopped.

## Terminal emulator recovery

`ree` reads standard and extended compiled terminfo entries without linking
to ncurses. It writes reset capabilities in this order:

1. `rs1`, or `is1` when `rs1` is absent
2. `rs2`, or `is2` when `rs2` is absent
3. `clear_margins`
4. `rs3`, or `is3` when `rs3` is absent

For a known VT-compatible terminal, `ree` first disables state that can
damage an interactive shell:

- synchronized output
- mouse tracking and mouse encoding modes
- focus reporting
- bracketed paste
- in-band size and terminal state reports
- Kitty paste events
- Kitty keyboard flags and keyboard stack entries
- xterm `modifyOtherKeys`

If the terminfo entry is absent, `ree` writes a fixed VT reset sequence. This
fallback supports an SSH connection from a recent terminal to a host that does
not have the terminal's terminfo entry.

## Ghostty and shell support

Ghostty normally sets `TERM=xterm-ghostty` and can provide its compiled entry
through `TERMINFO`. `ree` checks `TERMINFO` before the other terminfo
locations. It recognizes `xterm-ghostty` as a VT-compatible terminal and
disables Ghostty's reporting, paste, mouse, synchronized-output, and Kitty
keyboard modes before it writes the terminfo reset strings.

`ree` has no shell-specific code. Bash, Elvish, Fish, Nushell, and Zsh can run
the same binary. Nushell uses Reedline, which can enable bracketed paste and
Kitty keyboard flags while it reads input. The VT cleanup disables both
states.

## Examples of recoverable state

| Command or event | Terminal state |
| --- | --- |
| `cat /dev/urandom` | Escape sequences can change character sets or screen state |
| `printf '\e[?1049h'` | The terminal enters the alternate screen |
| `printf '\e[?25l'` | The cursor becomes hidden |
| `printf '\e[8m'` | Text becomes concealed |
| `printf '\e(0'` | The terminal selects DEC line-drawing characters |
| `printf '\e[?1003h\e[?1006h'` | Mouse events become input |
| `stty raw` | Line editing, echo, and signals stop |
| `stty -opost -onlcr` | Output no longer maps newlines correctly |

Run `ree` after one of these events. Some commands in the table use POSIX
shell syntax. The recovery command is the same in every supported shell.

## Limits

`ree` is designed for terminal emulators and pseudo-terminals. Use the system
`reset` command for a serial or physical terminal that needs hardware delays,
hardware tab-stop programming, or alternate margin handling.

`ree` differs from ncurses `reset` in these areas:

- It omits the historical one-second hardware settling delay.
- It does not program hardware tab stops.
- It does not read `reset_file` or `init_file` capabilities.
- It writes `clear_margins` when that capability is present.
- It does not use the ncurses alternate margin fallback.
- It removes terminfo padding markers instead of waiting or writing pad bytes.
- It adds a VT cleanup before the terminfo reset strings.
- It uses a fixed VT sequence when a terminfo entry is absent.

## Build

`ree` requires Rust 1.95 or later. The project uses Rust 2024 edition. The
selected Rust toolchain includes `rustfmt` and Clippy.

Build or install the host binary:

```text
just build-release
just install
```

Build all four release targets:

```text
just build-all
```

The Linux cross-builds require Zig and `cargo-zigbuild`. The complete checks
also require Bash, Just, Bun, `oxfmt`, `oxlint`, `fd`, Ripgrep, and Python 3.

The release profile uses size optimization, fat link-time optimization, one
code-generation unit, abort-on-panic, and symbol stripping. The size gate
limits each release binary to 600,000 bytes. The Linux ABI gate rejects a
binary that requires a glibc version later than 2.17.

## Test

Run the local release checks:

```text
just verify
```

The local checks include formatting, compilation, Clippy with warnings denied,
unit tests, documentation tests, a pseudo-terminal recovery test, and the host
binary size limit.

Run the cross-platform and package checks:

```text
just verify-release
```

This command builds all four release binaries, checks their sizes and Linux ABI
versions, and performs Cargo and npm publish dry runs. It does not publish a
package.

## Publish

[`Cargo.toml`](Cargo.toml) is the version source for the Cargo package and all
five npm packages. After you change the version, run `cargo check` to update
`Cargo.lock`.

```text
just verify-release
just publish-cargo
just publish-npm
```

`just publish-npm` publishes the four native packages before the root
`@seanmozeik/ree` package.

## Changes from rst

This fork retains the reset model, terminfo capability order, missing-entry VT
fallback, control-character preservation, and Apache-2.0 license from `rst`.

The Rust implementation adds these checks and recovery paths:

- exact matching for supported terminal families
- bounded and validated terminfo parsing
- error retention across terminfo search locations
- validation of terminfo padding markers
- mandatory foreground process-group verification
- Ghostty and Nushell terminal mode cleanup
- deterministic pseudo-terminal tests with time limits
- release checks for binary size and the Linux glibc baseline
- Cargo and npm packages for four native targets

## Related implementations

- [rauchg/rst](https://github.com/rauchg/rst) is the direct upstream project.
- [BusyBox reset](https://git.busybox.net/busybox/tree/console-tools/reset.c)
  restores terminal behavior with a fixed reset sequence.
- [Toybox reset](https://github.com/landley/toybox/blob/master/toys/other/reset.c)
  repairs the terminal driver state and writes fixed escape sequences.
- ncurses provides the system `reset` and `tput reset` implementations.

## License

`ree` is available under the Apache License 2.0. See [LICENSE](LICENSE).

#!/usr/bin/env python3
"""Exercise ree's real job-control, termios, terminfo, and VT recovery paths."""

import copy
import errno
import os
import pty
import struct
import subprocess
import sys
import tempfile
import termios

TIMEOUT_SECONDS = 5
VT_CLEANUP_START = b"\x1b]\x1b\\\x1b[?2026l"
VT_MODES_OFF = (
    b"\x1b[?9;1000;1002;1003;1004;1005;1006;1015;1016;"
    b"2004;2031;2033;2048;5522l"
)
KITTY_KEYBOARD_OFF = b"\x1b[<8u\x1b[=0u"
MODIFY_OTHER_KEYS_OFF = b"\x1b[>4;0m"
STRING_END_AND_RIS = b"\x1b]\x1b\\\x1bc"

REQUIRED_INPUT = ("BRKINT", "IGNPAR", "ICRNL", "IXON")
REQUIRED_OUTPUT = ("OPOST", "ONLCR")
REQUIRED_LOCAL = ("ISIG", "ICANON", "IEXTEN", "ECHO", "ECHOE", "ECHOK")
REQUIRED_CONTROL_CHARACTERS = (
    "VEOF",
    "VERASE",
    "VWERASE",
    "VKILL",
    "VREPRINT",
    "VINTR",
    "VQUIT",
    "VSUSP",
    "VSTART",
    "VSTOP",
    "VLNEXT",
    "VDISCARD",
)


def fail(message):
    print(f"FAIL: {message}", file=sys.stderr)
    raise SystemExit(1)


def run_checked(command, **kwargs):
    try:
        return subprocess.run(
            command,
            check=False,
            timeout=TIMEOUT_SECONDS,
            **kwargs,
        )
    except subprocess.TimeoutExpired:
        fail(f"{' '.join(command)} timed out after {TIMEOUT_SECONDS}s")


def assert_background_refusal(ree_path, env):
    result = run_checked(
        [ree_path],
        env=env,
        stdin=None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        preexec_fn=os.setpgrp,
    )
    if result.returncode != 1 or result.stdout or result.stderr:
        fail("background ree was not refused silently")


def assert_unowned_tty_refusal(ree_path, env):
    master, slave = pty.openpty()
    try:
        result = run_checked(
            [ree_path],
            env=env,
            stdin=slave,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            preexec_fn=os.setsid,
        )
    finally:
        os.close(slave)
        os.close(master)
    if result.returncode != 1 or result.stdout or result.stderr:
        fail("a process without terminal ownership was not refused silently")


def control_character_value(value):
    return value[0] if isinstance(value, bytes) else value


def assert_repaired(attrs):
    for name in REQUIRED_INPUT:
        if not attrs[0] & getattr(termios, name):
            fail(f"ree did not restore {name}")
    for name in REQUIRED_OUTPUT:
        if not attrs[1] & getattr(termios, name):
            fail(f"ree did not restore {name}")
    for name in REQUIRED_LOCAL:
        if not attrs[3] & getattr(termios, name):
            fail(f"ree did not restore {name}")
    if attrs[2] & termios.CSIZE != termios.CS8 or not attrs[2] & termios.CREAD:
        fail("ree did not restore CS8 and CREAD")
    for name in REQUIRED_CONTROL_CHARACTERS:
        index = getattr(termios, name)
        if control_character_value(attrs[6][index]) == 0:
            fail(f"ree did not restore {name}")


def run_recovery_case(fd, ree_path, env, label):
    original = termios.tcgetattr(fd)
    broken = copy.deepcopy(original)
    broken[0] = 0
    broken[1] = 0
    broken[3] = 0
    broken[6] = [0] * len(broken[6])
    termios.tcsetattr(fd, termios.TCSANOW, broken)

    current = termios.tcgetattr(fd)
    if current[3] & termios.ICANON or current[3] & termios.ECHO:
        fail(f"{label} setup did not disable canonical mode and echo")

    result = run_checked(
        [ree_path],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        termios.tcsetattr(fd, termios.TCSANOW, original)
        fail(f"ree exited {result.returncode} in {label}")

    assert_repaired(termios.tcgetattr(fd))
    termios.tcsetattr(fd, termios.TCSANOW, original)
    print(f"OK: {label}")


def write_ghostty_entry(root):
    term = b"xterm-ghostty"
    names = b"xterm-ghostty|ghostty|Ghostty\0"
    reset = STRING_END_AND_RIS + b"\0"
    offsets = [-1] * 123
    offsets[122] = 0
    header = struct.pack("<6H", 0o432, len(names), 0, 0, len(offsets), len(reset))
    padding = b"\0" if (len(header) + len(names)) % 2 else b""
    data = header + names + padding + struct.pack(f"<{len(offsets)}h", *offsets) + reset

    directory = os.path.join(root, f"{term[0]:02x}")
    os.makedirs(directory)
    with open(os.path.join(directory, term.decode()), "wb") as entry:
        entry.write(data)


def child_main(ree_path, terminfo_root):
    ghostty_env = os.environ.copy()
    ghostty_env.update(
        HOME=terminfo_root,
        TERM="xterm-ghostty",
        TERMINFO=terminfo_root,
        TERMINFO_DIRS=terminfo_root,
    )
    missing_env = ghostty_env.copy()
    missing_env["TERM"] = "xterm-ree-guaranteed-missing"

    assert_background_refusal(ree_path, ghostty_env)
    assert_unowned_tty_refusal(ree_path, ghostty_env)

    fd = os.open(os.ctermid(), os.O_RDWR)
    try:
        run_recovery_case(fd, ree_path, ghostty_env, "xterm-ghostty terminfo reset")
        run_recovery_case(fd, ree_path, missing_env, "missing terminfo VT fallback")
    finally:
        os.close(fd)
    raise SystemExit(0)


def assert_output_contract(output):
    required = {
        "VT cleanup start": VT_CLEANUP_START,
        "modern VT modes": VT_MODES_OFF,
        "Kitty keyboard reset": KITTY_KEYBOARD_OFF,
        "modifyOtherKeys reset": MODIFY_OTHER_KEYS_OFF,
    }
    for label, sequence in required.items():
        if output.count(sequence) < 2:
            fail(f"{label} was not emitted for both recovery paths")
    if output.count(STRING_END_AND_RIS) < 2:
        fail("RIS was not emitted by both Ghostty and fallback reset paths")


def main():
    ree_path = os.path.abspath(
        sys.argv[1] if len(sys.argv) > 1 else os.path.join("target", "release", "ree")
    )
    if not os.path.exists(ree_path):
        print(f"FAIL: ree binary not found at {ree_path}", file=sys.stderr)
        raise SystemExit(2)

    with tempfile.TemporaryDirectory(prefix="ree-terminfo-") as terminfo_root:
        write_ghostty_entry(terminfo_root)
        pid, fd = pty.fork()
        if pid == 0:
            child_main(ree_path, terminfo_root)
            return

        output = bytearray()
        try:
            while data := os.read(fd, 4096):
                output.extend(data)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
        finally:
            os.close(fd)

        _, status = os.waitpid(pid, 0)
        returncode = os.waitstatus_to_exitcode(status)
        if returncode != 0:
            fail(f"PTY child exited {returncode}; captured output: {bytes(output)!r}")
        assert_output_contract(output)

    print("pty test passed")


if __name__ == "__main__":
    main()

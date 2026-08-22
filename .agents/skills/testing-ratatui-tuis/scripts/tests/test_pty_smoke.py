import importlib.machinery
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "pty-smoke"
CHILD = r'''
import fcntl, os, struct, sys, termios
rows, cols, _, _ = struct.unpack("HHHH", fcntl.ioctl(sys.stdin.fileno(), termios.TIOCGWINSZ, b"\0" * 8))
sys.stdout.write(f"SIZE={cols}x{rows}\n\x1b[?1049hREADY\n")
sys.stdout.flush()
received = os.read(sys.stdin.fileno(), 1)
sys.stdout.write(f"KEY={received.decode()}\n\x1b[?1049l")
sys.stdout.flush()
'''
BYTE_CHILD = r'''
import os, sys
received = os.read(sys.stdin.fileno(), 1)
sys.stdout.write(f"BYTE={received[0]}\n")
sys.stdout.flush()
'''
DESCENDANT_CHILD = r'''
import pathlib, subprocess, sys, time
pid_path = pathlib.Path(sys.argv[1])
descendant = subprocess.Popen(
    [sys.executable, "-c", "import time; time.sleep(30)"],
)
pid_path.write_text(str(descendant.pid), encoding="utf-8")
time.sleep(30)
'''
NOISY_CHILD = r'''
import sys
sys.stdout.buffer.write(b"x" * 1_100_000)
sys.stdout.flush()
'''


def load_runner_module():
    loader = importlib.machinery.SourceFileLoader("pty_smoke", str(SCRIPT))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def process_exists(pid):
    result = subprocess.run(
        ["ps", "-p", str(pid), "-o", "pid="],
        check=False,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and bool(result.stdout.strip())


def terminate_pid(pid):
    try:
        os.kill(pid, 15)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + 1
    while process_exists(pid) and time.monotonic() < deadline:
        time.sleep(0.02)


class PtySmokeTests(unittest.TestCase):
    def write_scenario(self, directory, scenario):
        path = pathlib.Path(directory) / "scenario.json"
        path.write_text(json.dumps(scenario), encoding="utf-8")
        return path

    def run_runner(self, scenario_path, output_ansi, output_result, command, timeout=None):
        return subprocess.run(
            [
                str(SCRIPT),
                "--scenario",
                str(scenario_path),
                "--output-ansi",
                str(output_ansi),
                "--output-result",
                str(output_result),
                "--",
                *command,
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
        )

    def test_runs_child_at_requested_size_and_records_ansi_lifecycle(self):
        scenario = {
            "cols": 91,
            "rows": 27,
            "timeout_ms": 3000,
            "steps": [{"after_ms": 100, "send": "q"}],
            "expect_exit": 0,
            "require_alternate_screen": True,
        }
        with tempfile.TemporaryDirectory() as directory:
            scenario_path = self.write_scenario(directory, scenario)
            output_ansi = pathlib.Path(directory) / "session.ansi"
            output_result = pathlib.Path(directory) / "result.json"
            result = self.run_runner(
                scenario_path, output_ansi, output_result, [sys.executable, "-c", CHILD]
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            raw = output_ansi.read_bytes()
            recorded = json.loads(output_result.read_text(encoding="utf-8"))

        self.assertIn(b"SIZE=91x27", raw)
        self.assertIn(b"READY", raw)
        self.assertIn(b"KEY=q", raw)
        self.assertTrue(recorded["saw_enter_alternate_screen"])
        self.assertTrue(recorded["saw_leave_alternate_screen"])
        self.assertFalse(recorded["timed_out"])

    def test_times_out_sleeping_child(self):
        scenario = {"cols": 80, "rows": 24, "timeout_ms": 100, "steps": []}
        with tempfile.TemporaryDirectory() as directory:
            scenario_path = self.write_scenario(directory, scenario)
            output_ansi = pathlib.Path(directory) / "session.ansi"
            output_result = pathlib.Path(directory) / "result.json"
            result = self.run_runner(
                scenario_path,
                output_ansi,
                output_result,
                [sys.executable, "-c", "import time; time.sleep(10)"],
            )
            recorded = json.loads(output_result.read_text(encoding="utf-8"))

        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertTrue(recorded["timed_out"])
        self.assertEqual(recorded["exit_code"], 124)

    def test_delivers_named_enter_without_terminal_translation(self):
        scenario = {
            "cols": 80,
            "rows": 24,
            "timeout_ms": 3000,
            "steps": [{"after_ms": 10, "key": "enter"}],
            "expect_exit": 0,
        }
        with tempfile.TemporaryDirectory() as directory:
            scenario_path = self.write_scenario(directory, scenario)
            output_ansi = pathlib.Path(directory) / "session.ansi"
            output_result = pathlib.Path(directory) / "result.json"
            result = self.run_runner(
                scenario_path,
                output_ansi,
                output_result,
                [sys.executable, "-c", BYTE_CHILD],
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            raw = output_ansi.read_bytes()

        self.assertIn(b"BYTE=13", raw)

    def test_timeout_terminates_process_group_descendant(self):
        scenario = {"cols": 80, "rows": 24, "timeout_ms": 300, "steps": []}
        with tempfile.TemporaryDirectory() as directory:
            scenario_path = self.write_scenario(directory, scenario)
            output_ansi = pathlib.Path(directory) / "session.ansi"
            output_result = pathlib.Path(directory) / "result.json"
            descendant_path = pathlib.Path(directory) / "descendant.pid"
            result = self.run_runner(
                scenario_path,
                output_ansi,
                output_result,
                [sys.executable, "-c", DESCENDANT_CHILD, str(descendant_path)],
            )
            self.assertEqual(result.returncode, 124, result.stderr)
            descendant_pid = int(descendant_path.read_text(encoding="utf-8"))

            deadline = time.monotonic() + 2
            while process_exists(descendant_pid) and time.monotonic() < deadline:
                time.sleep(0.02)
            try:
                self.assertFalse(process_exists(descendant_pid))
            finally:
                terminate_pid(descendant_pid)

    def test_timeout_is_not_starved_by_continuous_pty_output(self):
        scenario = {"cols": 80, "rows": 24, "timeout_ms": 250, "steps": []}
        with tempfile.TemporaryDirectory() as directory:
            scenario_path = self.write_scenario(directory, scenario)
            output_ansi = pathlib.Path(directory) / "session.ansi"
            output_result = pathlib.Path(directory) / "result.json"
            child_pid_path = pathlib.Path(directory) / "writer.pid"
            started = time.monotonic()
            try:
                result = self.run_runner(
                    scenario_path,
                    output_ansi,
                    output_result,
                    [
                        "/bin/sh",
                        "-c",
                        'echo "$$" > "$1"; exec yes',
                        "pty-writer",
                        str(child_pid_path),
                    ],
                    timeout=1.5,
                )
            except subprocess.TimeoutExpired:
                self.fail("runner did not return while the PTY remained readable")
            finally:
                if child_pid_path.exists():
                    terminate_pid(int(child_pid_path.read_text(encoding="utf-8")))
            duration = time.monotonic() - started

            self.assertEqual(result.returncode, 124, result.stderr)
            recorded = json.loads(output_result.read_text(encoding="utf-8"))

        self.assertLess(duration, 1.0)
        self.assertTrue(recorded["timed_out"])
        self.assertTrue(recorded["transcript_truncated"])

    def test_truncates_noisy_transcript_at_documented_limit(self):
        runner = load_runner_module()
        scenario = {"cols": 80, "rows": 24, "timeout_ms": 3000, "steps": []}
        with tempfile.TemporaryDirectory() as directory:
            scenario_path = self.write_scenario(directory, scenario)
            output_ansi = pathlib.Path(directory) / "session.ansi"
            output_result = pathlib.Path(directory) / "result.json"
            result = self.run_runner(
                scenario_path,
                output_ansi,
                output_result,
                [sys.executable, "-c", NOISY_CHILD],
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            raw = output_ansi.read_bytes()
            recorded = json.loads(output_result.read_text(encoding="utf-8"))

        self.assertTrue(recorded["transcript_truncated"])
        self.assertEqual(len(raw), runner.MAX_TRANSCRIPT_BYTES)

    def test_limits_reads_per_iteration_when_pty_stays_readable(self):
        runner = load_runner_module()
        reads = 0
        original_read = runner.os.read

        def readable_until_budget(fd, size):
            nonlocal reads
            reads += 1
            if reads > 10:
                raise BlockingIOError()
            return b"x" * min(size, 1024)

        runner.os.read = readable_until_budget
        try:
            is_open, truncated = runner.read_available(1, bytearray())
        finally:
            runner.os.read = original_read

        self.assertTrue(is_open)
        self.assertFalse(truncated)
        self.assertLessEqual(reads, 4)

    def test_encodes_named_keys(self):
        runner = load_runner_module()
        expected = {
            "enter": b"\r",
            "esc": b"\x1b",
            "up": b"\x1b[A",
            "down": b"\x1b[B",
            "left": b"\x1b[D",
            "right": b"\x1b[C",
            "ctrl-e": b"\x05",
        }

        for key, encoded in expected.items():
            with self.subTest(key=key):
                self.assertEqual(runner.encode_step({"after_ms": 0, "key": key}), encoded)

    def test_does_not_import_process_table_for_timeout_cleanup(self):
        runner = load_runner_module()

        self.assertFalse(hasattr(runner, "descendant_pids"))
        self.assertFalse(hasattr(runner, "subprocess"))


if __name__ == "__main__":
    unittest.main()

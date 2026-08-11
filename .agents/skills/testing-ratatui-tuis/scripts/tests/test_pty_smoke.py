import importlib.machinery
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
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


def load_runner_module():
    loader = importlib.machinery.SourceFileLoader("pty_smoke", str(SCRIPT))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


class PtySmokeTests(unittest.TestCase):
    def write_scenario(self, directory, scenario):
        path = pathlib.Path(directory) / "scenario.json"
        path.write_text(json.dumps(scenario), encoding="utf-8")
        return path

    def run_runner(self, scenario_path, output_ansi, output_result, command):
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


if __name__ == "__main__":
    unittest.main()

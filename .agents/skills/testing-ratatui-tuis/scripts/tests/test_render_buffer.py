import importlib.machinery
import importlib.util
import json
import os
import pathlib
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ElementTree
from unittest import mock


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "render-buffer"
LOADER = importlib.machinery.SourceFileLoader("render_buffer", str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
RENDER_BUFFER = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(RENDER_BUFFER)
CHROMIUM = RENDER_BUFFER.find_chromium(None)


def capture():
    return {
        "version": 1,
        "width": 2,
        "height": 1,
        "cell_width": 10,
        "cell_height": 20,
        "default_fg": "#d8dee9",
        "default_bg": "#2e3440",
        "cells": [
            {
                "x": 0,
                "y": 0,
                "symbol": "A<&",
                "fg": "#ffffff",
                "bg": "#2e3440",
                "modifiers": ["bold"],
            },
            {
                "x": 1,
                "y": 0,
                "symbol": " ",
                "fg": "#d8dee9",
                "bg": "#bf616a",
                "modifiers": [],
            },
        ],
        "cursor": {"x": 0, "y": 0},
    }


class RenderBufferTests(unittest.TestCase):
    def write_capture(self, directory, value):
        path = pathlib.Path(directory) / "capture.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def render(self, input_path, svg_path, *extra):
        return subprocess.run(
            [str(SCRIPT), "--input", str(input_path), "--svg", str(svg_path), *extra],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_renders_deterministic_escaped_svg(self):
        value = capture()
        value["cells"][0]["symbol"] = "Ω<&"
        with tempfile.TemporaryDirectory() as directory:
            input_path = self.write_capture(directory, value)
            first_svg = pathlib.Path(directory) / "first.svg"
            second_svg = pathlib.Path(directory) / "second.svg"

            first = self.render(input_path, first_svg)
            second = self.render(input_path, second_svg)

            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first_svg.read_bytes(), second_svg.read_bytes())
            svg = first_svg.read_text(encoding="utf-8")
            self.assertIn('width="20" height="20"', svg)
            self.assertIn('viewBox="0 0 20 20"', svg)
            self.assertIn("Ω&lt;&amp;", svg)
            self.assertIn('font-weight="bold"', svg)
            self.assertIn('fill="#bf616a"', svg)
            self.assertIn('class="cursor"', svg)
            ElementTree.fromstring(svg)

    def test_renders_reversed_and_remaining_modifiers(self):
        value = capture()
        value["cells"][0].update(
            {
                "symbol": "R",
                "fg": "#112233",
                "bg": "#445566",
                "modifiers": [
                    "reversed",
                    "dim",
                    "italic",
                    "underlined",
                    "crossed_out",
                ],
            }
        )
        with tempfile.TemporaryDirectory() as directory:
            svg_path = pathlib.Path(directory) / "capture.svg"
            result = self.render(self.write_capture(directory, value), svg_path)

            self.assertEqual(result.returncode, 0, result.stderr)
            svg = svg_path.read_text(encoding="utf-8")

        self.assertIn('x="0" y="0" width="10" height="20" fill="#112233"', svg)
        self.assertIn('fill="#445566" opacity="0.6"', svg)
        self.assertIn('font-style="italic"', svg)
        self.assertIn('text-decoration="underline line-through"', svg)

    def test_rejects_boolean_version(self):
        value = capture()
        value["version"] = True
        with tempfile.TemporaryDirectory() as directory:
            result = self.render(
                self.write_capture(directory, value), pathlib.Path(directory) / "out.svg"
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, "error: version must be integer 1\n")

    def test_rejects_xml_forbidden_symbol_characters_without_writing_svg(self):
        for symbol, codepoint in (("\x01", "U+0001"), ("\ud800", "U+D800")):
            with self.subTest(codepoint=codepoint), tempfile.TemporaryDirectory() as directory:
                value = capture()
                value["cells"][0]["symbol"] = symbol
                svg_path = pathlib.Path(directory) / "out.svg"
                result = self.render(self.write_capture(directory, value), svg_path)

                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(
                    result.stderr,
                    f"error: cell symbol contains XML 1.0-forbidden character: {codepoint}\n",
                )
                self.assertFalse(svg_path.exists())

    def test_rejects_out_of_range_cell_coordinate(self):
        value = capture()
        value["cells"][0]["x"] = 2
        with tempfile.TemporaryDirectory() as directory:
            result = self.render(
                self.write_capture(directory, value), pathlib.Path(directory) / "out.svg"
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, "error: cell coordinate out of bounds: (2, 0)\n")

    def test_rejects_duplicate_cell_coordinate(self):
        value = capture()
        value["cells"][1]["x"] = 0
        with tempfile.TemporaryDirectory() as directory:
            result = self.render(
                self.write_capture(directory, value), pathlib.Path(directory) / "out.svg"
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, "error: duplicate cell coordinate: (0, 0)\n")

    def test_finds_executable_standard_macos_browser_when_path_candidates_are_absent(self):
        browsers = (
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        )
        for browser in browsers:
            with (
                self.subTest(browser=browser),
                mock.patch.object(RENDER_BUFFER.shutil, "which", return_value=None),
                mock.patch.object(
                    pathlib.Path, "is_file", lambda path: str(path) == browser
                ),
                mock.patch(
                    "os.access",
                    lambda path, mode: str(path) == browser and mode == os.X_OK,
                ),
                mock.patch.object(
                    RENDER_BUFFER.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([browser, "--version"], 0),
                ),
            ):
                self.assertEqual(RENDER_BUFFER.find_chromium(None), browser)

    def test_skips_broken_path_candidate_for_working_standard_macos_chrome(self):
        broken = "/opt/homebrew/bin/chromium"
        chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

        def which(name):
            return broken if name == "chromium" else None

        def probe(command, **options):
            self.assertLessEqual(options["timeout"], 5)
            return subprocess.CompletedProcess(
                command,
                126 if command[0] == broken else 0,
            )

        with (
            mock.patch.object(RENDER_BUFFER.shutil, "which", side_effect=which),
            mock.patch.object(
                pathlib.Path, "is_file", lambda path: str(path) == chrome
            ),
            mock.patch(
                "os.access",
                lambda path, mode: str(path) == chrome and mode == os.X_OK,
            ),
            mock.patch.object(RENDER_BUFFER.subprocess, "run", side_effect=probe),
        ):
            self.assertEqual(RENDER_BUFFER.find_chromium(None), chrome)

    @unittest.skipUnless(CHROMIUM, "Chromium is not installed")
    def test_rasterizes_svg_to_png_when_chromium_is_available(self):
        with tempfile.TemporaryDirectory() as directory:
            input_path = self.write_capture(directory, capture())
            svg_path = pathlib.Path(directory) / "capture.svg"
            png_path = pathlib.Path(directory) / "capture.png"
            result = self.render(
                input_path,
                svg_path,
                "--png",
                str(png_path),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(png_path.read_bytes()[:8], b"\x89PNG\r\n\x1a\n")


if __name__ == "__main__":
    unittest.main()

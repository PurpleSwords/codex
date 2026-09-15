"""Resource collection must not turn a failing build into a successful job."""

import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import macos_build_observer as observer


class BuildObserverTests(unittest.TestCase):
    def test_preserves_build_exit_status_even_when_probes_fail(self):
        for status in (0, 1, 101):
            with self.subTest(status=status):
                process = Mock()
                process.wait.return_value = status
                with (
                    patch.object(observer.subprocess, "Popen", return_value=process),
                    patch.object(
                        observer.subprocess,
                        "run",
                        side_effect=OSError("probe unavailable"),
                    ),
                ):
                    self.assertEqual(
                        observer.observe(["cargo", "build"], io.StringIO()), status
                    )
                process.terminate.assert_not_called()
                process.kill.assert_not_called()

    def test_long_build_is_observed_without_retrying_or_killing_it(self):
        process = Mock()
        process.wait.side_effect = [subprocess.TimeoutExpired("cargo", 60), 101]
        with (
            patch.object(observer.subprocess, "Popen", return_value=process) as spawn,
            patch.object(
                observer.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 0, stdout=""),
            ) as probe,
        ):
            self.assertEqual(observer.observe(["cargo", "build"], io.StringIO()), 101)
        spawn.assert_called_once_with(["cargo", "build"])
        self.assertEqual(probe.call_count, 6)
        process.terminate.assert_not_called()
        process.kill.assert_not_called()

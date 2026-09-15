#!/usr/bin/env python3
"""Observe a build without changing its exit status or terminating its processes."""

from datetime import datetime, timezone
import os
from pathlib import Path
import subprocess
import sys


def observe(command, stream):
    # Inherit build output so GitHub retains it even if the job times out.
    process = subprocess.Popen(command)
    while True:
        print(datetime.now(timezone.utc).isoformat(), file=stream, flush=True)
        for probe in (
            ["ps", "-axo", "pid,ppid,%cpu,rss,comm"],
            ["vm_stat"],
            ["sysctl", "vm.swapusage"],
        ):
            try:
                result = subprocess.run(
                    probe, capture_output=True, text=True, timeout=10
                )
                print(result.stdout, file=stream, flush=True)
                print(result.stdout, flush=True)
            except (OSError, subprocess.TimeoutExpired) as error:
                print(f"Resource probe failed: {error}", file=stream, flush=True)
        try:
            return process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            pass


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit("Expected a build command")
    with (Path(os.environ["RUNNER_TEMP"]) / "macos-build-resources.log").open(
        "w"
    ) as log:
        raise SystemExit(observe(sys.argv[1:], log))

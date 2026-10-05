#!/usr/bin/env python3
"""Sequential, disposable AMD/RADV/Vulkan dev/release acceptance on the normal client."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def isolated_env(base, directory):
    env = dict(base)
    env.update(CARGO_BUILD_JOBS="2", RUSTCRAFT_F1_OUTPUT=str(directory),
               RUSTCRAFT_SAVES_DIR=str(directory / "saves"),
               RUSTCRAFT_CONFIG_FILE=str(directory / "config-v1.json"),
               RUSTCRAFT_RESOURCE_CACHE=str(directory.parent / "resource-cache"),
               RUSTCRAFT_WORLD_NAME="f1-disposable")
    # Match the two runs; inherited operational/game settings still appear in effective config.
    # CLI overrides below control checkpoint cadence identically in both profiles.
    return env


def execute(command, env, log, timeout):
    with log.open("w", encoding="utf-8") as stream:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=stream,
                                stderr=subprocess.STDOUT, timeout=timeout, check=False)
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}); see {log}")


def load_profile(path, profile, manual=False):
    result = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(result, dict) or result.get("schema_version") != 1 or result.get("status") != "measured":
        raise ValueError(f"{profile}: invalid/incomplete F1 summary schema/status")
    if result.get("build_profile") != profile:
        raise ValueError(f"{profile}: actual build assertions/profile mismatch")
    info = result.get("adapter", {})
    if info.get("vendor") != 0x1002 or info.get("backend") != "Vulkan" or "radv" not in (
            info.get("driver", "") + " " + info.get("driver_info", "")).lower():
        raise ValueError(f"{profile}: summary is not AMD/RADV/Vulkan")
    if [p.get("phase") for p in result.get("phases", [])] != (["interactive"] if manual else ["stationary", "pan", "walk", "walk_pan", "fast_pan", "walk_fast_pan"]):
        raise ValueError(f"{profile}: missing/incorrect F1 phases")
    for phase in result["phases"]:
        if phase.get("frames", 0) == 0 or len(phase.get("render_ms", [])) < 6:
            raise ValueError(f"{profile}: phase has no frame/timing evidence")
    if not manual and result.get("player_publication_ms", {}).get("count", 0) < 20:
        raise ValueError(f"{profile}: insufficient checkpoint samples")
    timeline = json.loads((path.parent / "timeline.json").read_text(encoding="utf-8"))
    if not isinstance(timeline, dict) or timeline.get("schema_version") != 1 or timeline.get("dropped_trace_events") != 0:
        raise ValueError(f"{profile}: invalid/truncated timeline")
    return result


def run(profiles, manual=False):
    if manual and profiles != ["release"]:
        raise ValueError("interactive F1 recording requires release only")
    directory = ROOT / "target" / "f1" / f"run-{time.time_ns()}"
    directory.mkdir(parents=True)
    results = {}
    summary = {"schema_version": 1, "status": "running", "mode": "interactive" if manual else "automated", "profiles": results,
               "implementation_sha": subprocess.check_output(
                   ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
               "worktree_changed_paths": len(subprocess.check_output(
                   ["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()),
               "build_environment_overrides": {key: os.environ[key] for key in (
                   "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_DEV_OPT_LEVEL",
                   "CARGO_PROFILE_RELEASE_DEBUG", "CARGO_PROFILE_RELEASE_OPT_LEVEL",
                   "CARGO_INCREMENTAL") if key in os.environ},
               "limitations": "Application timing only; owner must report subjective feel separately; no scanout/input-to-photon claim"}
    write(directory / "summary.json", summary)
    try:
        for profile in profiles:
            output = directory / profile
            output.mkdir()
            env = isolated_env(os.environ, output)
            cargo = ["cargo", "run", "-p", "rustcraft-client"]
            if profile == "release":
                cargo.append("--release")
            # The application probes Vulkan before loading assets/opening a save.
            execute(cargo + ["--", "--f1-probe"], env, output / "probe.log", 1800)
            if manual:
                print("Click the normal client window to capture the mouse. Walk and use normal fast mouse-look for 45 seconds; recording then exits. Report subjective feel separately.", flush=True)
            execute(cargo + ["--", "--f1-manual" if manual else "--f1-acceptance", "--world", "f1-disposable",
                             "--survival", "--config-file", str(output / "config-v1.json"),
                             "--set-config", "rustcraft:persistence/player_interval_ms=1000",
                             "--set-config", "rustcraft:persistence/world_interval_ms=1000"],
                    env, output / "client.log", 600)
            result = load_profile(output / "summary.json", profile, manual)
            results[profile] = result
            print(f"F1 {profile}: measured; {output / 'summary.json'}", flush=True)
        if len(results) == 2:
            dev, release = results["dev"], results["release"]
            for field in ("world_seed", "generator_version"):
                if dev.get(field) != release.get(field):
                    raise ValueError(f"dev/release workload mismatch: {field}")
            for a, b in zip(dev["phases"], release["phases"]):
                for field in ("window_pixels", "present_mode", "backend"):
                    if a.get(field) != b.get(field):
                        raise ValueError(f"dev/release {a['phase']} mismatch: {field}")
                effective = lambda phase: {k: v["effective"] for k, v in
                                          phase["configuration"]["settings"].items()}
                if effective(a) != effective(b):
                    raise ValueError(f"dev/release {a['phase']} effective configuration mismatch")
        summary["status"] = "measured"
    except (OSError, RuntimeError, subprocess.TimeoutExpired, ValueError, KeyError, TypeError, AttributeError) as error:
        summary.update(status="failed", error=str(error))
        # Preserve the application error rather than requiring raw log interpretation.
        if (output / "summary.json").exists():
            try:
                summary["application_failure"] = json.loads((output / "summary.json").read_text(encoding="utf-8"))
            except ValueError:
                summary["application_failure"] = {"error": "malformed application JSON", "path": str(output / "summary.json")}
        print(f"F1 failed: {error}", file=sys.stderr)
    write(directory / "summary.json", summary)
    print(f"F1_SUMMARY {directory / 'summary.json'} {summary['status']}", flush=True)
    return 0 if summary["status"] == "measured" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", nargs="?", choices=("both", "dev", "release"), default="both")
    parser.add_argument("--manual", action="store_true", help="45-second owner-controlled release recording")
    args = parser.parse_args()
    if args.manual and args.profile != "release":
        parser.error("--manual requires release")
    return run(["dev", "release"] if args.profile == "both" else [args.profile], args.manual)


if __name__ == "__main__":
    sys.exit(main())

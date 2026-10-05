"""Safety and failure evidence for the field wrapper, independent of GPU/builds."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("f1_client", Path(__file__).parents[1] / "f1_client.py")
f1 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(f1)


class F1WorkflowTests(unittest.TestCase):
    def test_manual_rejects_dev_before_any_build(self):
        with self.assertRaises(ValueError):
            f1.run(["dev"], manual=True)

    def test_isolation_replaces_ordinary_saves_and_settings(self):
        path = Path("/tmp/disposable-f1/dev")
        env = f1.isolated_env({"RUSTCRAFT_SAVES_DIR": "/ordinary", "RUSTCRAFT_CONFIG_FILE": "/settings",
                               "RUSTCRAFT_TERRAIN_TEXTURE": "/local/terrain.png"}, path)
        self.assertEqual(env["RUSTCRAFT_SAVES_DIR"], str(path / "saves"))
        self.assertEqual(env["RUSTCRAFT_CONFIG_FILE"], str(path / "config-v1.json"))
        self.assertEqual(env["RUSTCRAFT_TERRAIN_TEXTURE"], "/local/terrain.png")
        self.assertEqual(env["CARGO_BUILD_JOBS"], "2")

    def test_probe_failure_stops_before_world_run_and_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = []
            def failure(command, env, log, timeout):
                calls.append(command)
                f1.write(Path(env["RUSTCRAFT_F1_OUTPUT"]) / "summary.json",
                         {"status": "failed", "error": "no Vulkan adapter"})
                raise RuntimeError("probe failed")
            with patch.object(f1, "ROOT", root), patch.object(f1, "execute", failure), \
                    patch.object(f1.subprocess, "check_output", return_value="test-sha\n"):
                self.assertEqual(f1.run(["dev", "release"]), 1)
            self.assertEqual(len(calls), 1)
            self.assertIn("--f1-probe", calls[0])
            summary = next((root / "target" / "f1").glob("run-*/summary.json"))
            import json
            result = json.loads(summary.read_text())
            self.assertEqual(result["application_failure"]["error"], "no Vulkan adapter")
            self.assertFalse(list((root / "target" / "f1").glob("run-*/release")))

    def test_malformed_or_nonrepresentative_summary_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "summary.json"
            for value in [{}, {"schema_version": 1, "status": "measured", "build_profile": "dev",
                              "adapter": {"vendor": 0, "backend": "Vulkan", "driver": "llvmpipe"}}]:
                f1.write(path, value)
                with self.assertRaises(ValueError):
                    f1.load_profile(path, "dev")
            path.write_text("{broken")
            with self.assertRaises(ValueError):
                f1.load_profile(path, "dev")

    def test_manual_runs_only_release_and_validates_interactive_trace(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = []
            def measured(command, env, log, timeout):
                calls.append(command)
                output = Path(env["RUSTCRAFT_F1_OUTPUT"])
                self.assertIn("--release", command)
                self.assertEqual(env["RUSTCRAFT_SAVES_DIR"], str(output / "saves"))
                if "--f1-manual" not in command:
                    return
                f1.write(output / "summary.json", {
                    "schema_version": 1, "status": "measured", "build_profile": "release",
                    "adapter": {"vendor": 0x1002, "backend": "Vulkan", "driver": "radv"},
                    "phases": [{"phase": "interactive", "frames": 300,
                                "render_ms": [300, 16, 16, 16, 16, 16]}]})
                f1.write(output / "timeline.json", {"schema_version": 1, "dropped_trace_events": 0})
            with patch.object(f1, "ROOT", root), patch.object(f1, "execute", measured), \
                    patch.object(f1.subprocess, "check_output", return_value="test-sha\n"):
                self.assertEqual(f1.run(["release"], manual=True), 0)
            self.assertEqual(len(calls), 2)
            self.assertIn("--f1-probe", calls[0])
            self.assertIn("--f1-manual", calls[1])
            self.assertNotIn("--f1-acceptance", calls[1])

    def test_profiles_execute_sequentially_and_publish_one_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls = []
            def measured(command, env, log, timeout):
                calls.append((Path(env["RUSTCRAFT_F1_OUTPUT"]).name, command[-1]))
                output = Path(env["RUSTCRAFT_F1_OUTPUT"])
                profile = output.name
                if "--f1-acceptance" not in command:
                    return
                phases = [{"phase": name, "frames": 400, "render_ms": [400, 16, 16, 16, 16, 16],
                           "window_pixels": [1280, 720], "present_mode": "Fifo", "backend": "Vulkan",
                           "configuration": {"settings": {"key": {"effective": 1000}}}}
                          for name in ["stationary", "pan", "walk", "walk_pan", "fast_pan", "walk_fast_pan"]]
                f1.write(output / "summary.json", {"schema_version": 1, "status": "measured",
                         "build_profile": profile, "adapter": {"vendor": 0x1002, "backend": "Vulkan",
                         "driver": "radv"}, "phases": phases, "player_publication_ms": {"count": 30},
                         "world_seed": 731173, "generator_version": 2})
                f1.write(output / "timeline.json", {"schema_version": 1, "dropped_trace_events": 0})
            with patch.object(f1, "ROOT", root), patch.object(f1, "execute", measured), \
                    patch.object(f1.subprocess, "check_output", return_value="test-sha\n"):
                self.assertEqual(f1.run(["dev", "release"]), 0)
            self.assertEqual([profile for profile, _ in calls], ["dev", "dev", "release", "release"])
            summary = next((root / "target" / "f1").glob("run-*/summary.json"))
            import json
            self.assertEqual(set(json.loads(summary.read_text())["profiles"]), {"dev", "release"})


if __name__ == "__main__":
    unittest.main()

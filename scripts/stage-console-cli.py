"""Validate an oxvif binary and stage it for dx bundle on the current host."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def check_cli(executable):
    env = os.environ.copy()
    for variable in ("OXVIF_USERNAME", "OXVIF_PASSWORD", "OXVIF_DEVICE"):
        env.pop(variable, None)
    result = subprocess.run(
        [executable, "describe", "--grammar", "--output", "json", "--non-interactive"],
        check=True,
        capture_output=True,
        env=env,
        timeout=10,
    )
    grammar = json.loads(result.stdout)
    data = grammar["data"]
    if (
        not grammar["ok"]
        or grammar["schema_version"] != "3"
        or data["grammar_version"] != 1
        or data["event_schema_version"] != "3"
    ):
        raise ValueError("Incompatible oxvif grammar")

    def commands(node):
        if node.get("console") is not None:
            yield node["command_id"], node["console"]
        for child in node["subcommands"]:
            yield from commands(child)

    supported = dict(commands(data["root"]))
    for name, output in (
        ("device.info", "json"),
        ("media.profiles", "json"),
        ("events.watch", "jsonl"),
    ):
        command = supported[name]
        if command["target_flag"] != "--target" or command["output"] != output:
            raise ValueError(f"Incompatible console command {name}")
    control = supported["events.watch"]["control"]
    if control["kind"] != "stdin_stop_v1" or control["flag"] != "--control-stdin":
        raise ValueError("Incompatible oxvif watch control")
    for name, target_flag in (("diagnose", "--target"), ("config.validate", None)):
        command = supported.get(name, {})
        if (
            command.get("share_report_version") != 1
            or command.get("output") != "json"
            or command.get("target_flag") != target_flag
        ):
            raise ValueError(f"Missing v1 console share report support for {name}")
    for name in ("setup", "auth", "device.credential.set", "credential.profile.set"):
        if supported.get(name, {}).get("credential_input") != "password_stdin_v1":
            raise ValueError(f"Missing console credential input support for {name}")
    manage = next((node for node in data["root"]["subcommands"] if node["command_id"] == "manage"), {})
    if manage.get("external_terminal") != "manage_v1":
        raise ValueError("Missing external terminal support for manage")
    print(f"oxvif {data['cli_version']}: {executable}")


def extract_msi(bundle, payload):
    subprocess.run(
        ["msiexec.exe", "/a", bundle, "/qn", f"TARGETDIR={payload}"],
        check=True,
        timeout=120,
    )


def check_bundle(bundle, mock):
    with tempfile.TemporaryDirectory(prefix="oxdm-bundle-") as work:
        payload = Path(work) / "payload"
        payload.mkdir()
        if bundle.suffix == ".msi":
            extract_msi(bundle, payload)
        elif bundle.suffix == ".deb":
            subprocess.run(["dpkg-deb", "-x", bundle, payload], check=True, timeout=120)
        elif bundle.suffix == ".dmg":
            subprocess.run(
                ["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", payload, bundle],
                check=True,
                timeout=120,
            )
        else:
            raise ValueError(f"Unsupported package: {bundle.suffix}")
        try:
            suffix = ".exe" if bundle.suffix == ".msi" else ""
            executables = [
                app.with_name(f"oxdm-oxvif{suffix}")
                for app in payload.rglob(f"oxdm{suffix}")
                if app.is_file()
            ]
            if len(executables) != 1 or not executables[0].is_file():
                raise ValueError("Expected oxdm and its CLI sidecar in the same directory")
            executable = executables[0]
            if os.name != "nt" and not os.access(executable, os.X_OK):
                raise ValueError(f"Bundled sidecar is not executable: {executable}")
            check_cli(executable)
            subprocess.run(
                ["cargo", "test", "--locked", "console::tests::", "--", "--ignored"],
                env={
                    **os.environ,
                    "OXVIF_CONFIG_DIR": str(Path(work) / "oxvif-config"),
                    "OXDM_OXVIF_TEST": str(executable),
                    "OXDM_MOCK_EVENTS_TEST": str(mock),
                },
                check=True,
            )
        finally:
            if bundle.suffix == ".dmg":
                subprocess.run(["hdiutil", "detach", payload], check=True, timeout=120)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check-only", action="store_true")
    mode.add_argument("--bundle", action="store_true")
    mode.add_argument("--blind-test", action="store_true")
    parser.add_argument("--mock", type=Path, help="Matching console_mock test fixture")
    args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    if args.bundle or args.blind_test:
        if args.mock is None:
            parser.error("--bundle and --blind-test require --mock")
        mock = args.mock.resolve(strict=True)
        if args.blind_test:
            if os.name != "nt" or executable.suffix.lower() != ".msi":
                parser.error("--blind-test requires a Windows MSI")
            blind_test(executable, mock)
        else:
            check_bundle(executable, mock)
        return
    check_cli(executable)
    if args.check_only:
        return
    rustc = subprocess.check_output(["rustc", "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    suffix = ".exe" if os.name == "nt" else ""
    root = Path(__file__).resolve().parents[1]
    stage = root / "target" / "console-sidecar" / f"oxdm-oxvif{suffix}-{host}"
    stage.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, stage)
    if os.name != "nt":
        stage.chmod(0o755)
    print(f"Staged {stage}")


def blind_test(bundle, mock):
    with tempfile.TemporaryDirectory(prefix="oxdm-blind-", ignore_cleanup_errors=True) as work:
        root = Path(work)
        payload = root / "payload"
        payload.mkdir()
        extract_msi(bundle, payload)
        apps = list(payload.rglob("oxdm.exe"))
        if len(apps) != 1:
            raise ValueError("Expected one OxDM application in the MSI")
        env = os.environ.copy()
        for variable in (
            "OXDM_OXVIF", "OXVIF_USERNAME", "OXVIF_PASSWORD", "OXVIF_DEVICE",
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER",
        ):
            env.pop(variable, None)
        env["OXDM_CONFIG_DIR"] = str(root / "profile")
        env["OXVIF_CONFIG_DIR"] = str(root / "cli")
        env["WEBVIEW2_USER_DATA_FOLDER"] = str(root / "webview")
        check_cli(apps[0].with_name("oxdm-oxvif.exe"))
        with subprocess.Popen(
            [mock], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
            creationflags=subprocess.CREATE_NO_WINDOW,
        ) as camera:
            try:
                address = camera.stdout.readline().strip()
                if not address.startswith("http://127.0.0.1:"):
                    raise ValueError("The mock camera did not return a loopback URL")
                profile = Path(env["OXDM_CONFIG_DIR"])
                profile.mkdir()
                (profile / "devices.toml").write_text(
                    f'[[devices]]\nname = "Local test camera"\naddr = {json.dumps(address)}\n',
                    encoding="utf-8",
                )
                camera.stdin.write("burst\n")
                camera.stdin.flush()
                print(f"Local test camera: {address}\nOxDM: {apps[0]}", flush=True)
                subprocess.run([apps[0]], cwd=apps[0].parent, env=env, check=True)
            finally:
                camera.stdin.close()
                try:
                    camera.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    camera.kill()
                    camera.wait()
                # WebView2 can hold profile files briefly after OxDM exits.
                deadline = time.monotonic() + 10
                while root.exists():
                    try:
                        shutil.rmtree(root)
                    except FileNotFoundError:
                        break
                    except PermissionError:
                        if time.monotonic() >= deadline:
                            print(f"WebView2 still holds test files in {root}", flush=True)
                            break
                        time.sleep(0.25)


if __name__ == "__main__":
    main()

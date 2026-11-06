"""Use Cargo's workspace member paths and normalized manifests for Git crates.

flatpak-cargo-generator identifies Git crates by name alone. Xberg ships an
unrelated fixture named xberg as well as the actual workspace member.
"""

import json
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path


def output(*args):
    return subprocess.check_output(args, text=True)


def main(manifest, source_file):
    sources = json.loads(source_file.read_text())
    git_sources = {s["commit"]: s["dest"] for s in sources if s["type"] == "git"}
    if not git_sources:
        return

    metadata = output("cargo", "metadata", "--locked", "--format-version", "1", "--manifest-path", manifest)
    packages = json.loads(metadata)["packages"]
    with tempfile.TemporaryDirectory() as tmp:
        vendor = Path(tmp) / "vendor"
        # Cargo resolves workspace.package and workspace.dependencies, including
        # paths inherited from the upstream root rather than the crate itself.
        output("cargo", "vendor", "--quiet", "--locked", "--versioned-dirs", "--manifest-path", manifest, vendor)
        for package in packages:
            source = package["source"] or ""
            if not source.startswith("git+"):
                continue
            name, version = package["name"], package["version"]
            commit = source.rsplit("#", 1)[1]
            crate = Path(package["manifest_path"]).parent
            checkout = Path(output("git", "-C", crate, "rev-parse", "--show-toplevel").strip())
            origin = Path(git_sources[commit]) / crate.relative_to(checkout)
            canonical = vendor / f"{name}-{version}"
            dest = f"cargo/vendor/{name}"

            # The pinned generator emits one copy command, then two inline
            # files for each Git crate. Fail if that contract changes.
            [copy] = [
                s for s in sources
                if s["type"] == "shell" and any(shlex.split(cmd)[-1] == dest for cmd in s["commands"])
            ]
            copy["commands"] = [f"cp -r --reflink=auto {shlex.quote(str(origin))} {shlex.quote(dest)}"]
            for filename in ("Cargo.toml", ".cargo-checksum.json"):
                [inline] = [s for s in sources if s.get("dest") == dest and s.get("dest-filename") == filename]
                inline["contents"] = (canonical / filename).read_text()

    source_file.write_text(json.dumps(sources, indent=4) + "\n")


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))

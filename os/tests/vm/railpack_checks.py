"""Build and run synthetic language projects through the installed Railpack owner."""
from functools import partial
import json
import shlex
import uuid
from build_checks import (
    build_request, exported_build, build_diagnostics, released_diagnostics, console_build_diagnostics,
)


PROJECTS = {
    'node': {
        'package.json': json.dumps({'name': 'anima-railpack-node', 'version': '1.0.0',
                                   'scripts': {'start': 'node index.js'}, 'engines': {'node': '22'}}),
        'index.js': 'console.log("anima-railpack-node");\n',
    },
    'python': {'requirements.txt': '', 'main.py': 'print("anima-railpack-python")\n'},
    'rust': {
        'Cargo.toml': '[package]\nname = "anima-railpack-rust"\nversion = "0.1.0"\nedition = "2021"\n',
        'src/main.rs': 'fn main() { println!("anima-railpack-rust"); }\n',
    },
}


BUILD_MEMORY_MIB = {"node": 512, "python": 512, "rust": 2048}


class BuildFixture:
    def __init__(self, machine):
        self.machine = machine
        self.workspace, self.context, self.attempt = (str(uuid.uuid4()) for _ in range(3))
        self.receipts = f'/run/anima-railpack-check-{self.attempt}'
        self.unit = f'anima-build-{self.attempt}.service'
        self.request = partial(build_request, machine, self.receipts)

    def start(self, files, memory_mib):
        self.machine.command(f'mkdir -m 700 {self.receipts}')
        self.request('prepare', 'prepare', '.value.phase == "prepared"',
                     workspace=self.workspace, storage_bytes=1024**3)
        project = f'/persist/anima-workspaces/homes/{self.workspace}/workspace'
        for name, content in files.items():
            path = shlex.quote(f'{project}/{name}')
            parent = shlex.quote(f'{project}/{name.rpartition("/")[0]}')
            self.machine.command(f"mkdir -p {parent}; printf '%s' {shlex.quote(content)} > {path}")
        self.machine.command(f'chown -R 1000:1000 {project}')
        self.request('capture', 'capture', '.value.phase == "captured"',
                     workspace=self.workspace, generation=0, context=self.context)
        self.request('launch', 'build', '.value.phase == "running" and .value.strategy == "railpack"',
                     attempt=self.attempt, workspace=self.workspace, generation=0, context=self.context,
                     strategy='railpack',
                     resources={'memory_bytes': memory_mib*1024**2, 'cpu_millis': 500, 'tasks': 128})

    def wait(self, seconds):
        self.machine.command(
            f"""timeout {seconds} sh -c 'while systemctl is-active --quiet {self.unit}; do sleep 2; done'""",
            timeout=seconds+10)

    def release(self):
        self.request('release', 'build-release', '.value.phase == "deleted"', attempt=self.attempt, confirmed=True)
        released_diagnostics(self.machine, self.receipts, self.attempt)
        self.request('context-release', 'context-release', '.value.phase == "deleted"',
                     workspace=self.workspace, generation=0, context=self.context, confirmed=True)
        self.request('workspace-release', 'workspace-release',
                     '.value.phase == "deleted" and .value.storage_bytes == 0',
                     workspace=self.workspace, generation=0, confirmed=True)

    def diagnose(self):
        try:
            self.request('failure-status', 'build-status', '.value.attempt.id == ' + json.dumps(self.attempt),
                         attempt=self.attempt)
        except Exception:
            pass
        try:
            self.machine.command(f'journalctl -b -u {self.unit} --no-pager -n 40')
        except Exception:
            pass


def installed_railpack(machine, language):
    build = BuildFixture(machine)
    try:
        build.start(PROJECTS[language], BUILD_MEMORY_MIB[language])
        build.wait(3900)
        build.request('verified', 'build-reconcile',
                '.value.phase == "verified" and .value.result.integrityVerified and (.value.result.deployed | not)',
                attempt=build.attempt)
        receipts = build.receipts
        build_diagnostics(machine, receipts, build.attempt)
        exported_build(machine, receipts)
        # Test-only deployment: production build completion must never perform this import.
        machine.command(f'docker load -i {receipts}/image.oci > {receipts}/load.log', timeout=180)
        machine.command(f"manifest=$(jq -er '[.manifests[] | "
                        f"select(.platform.os == \"linux\" and .platform.architecture == \"amd64\") | "
                        f"select(.annotations[\"vnd.docker.reference.type\"] != \"attestation-manifest\")] | "
                        f"if length == 1 then .[0].digest else error(\"Expected one runnable image\") end' "
                        f'{receipts}/index.json) || exit 1; '
                        f'tar -xOf {receipts}/image.oci "blobs/sha256/${{manifest#sha256:}}" '
                        f'> {receipts}/runnable.json || exit 1; '
                        f"digest=$(jq -er '.value.result.manifestDigest' {receipts}/verified.json) || exit 1; "
                        f'image="localhost/anima/build@$digest"; '
                        f'docker image inspect "$image" > {receipts}/loaded.json || exit 1; '
                        f"jq -e --arg image \"$image\" '.[0].RepoDigests | index($image) != null' "
                        f'{receipts}/loaded.json >/dev/null || exit 1; '
                        f'docker run --rm --pull never --platform linux/amd64 --network none --read-only --memory 256m --cpus 1 '
                        f'--cap-drop ALL --security-opt no-new-privileges "$image" > {receipts}/run.log || exit 1; '
                        f'grep -qx anima-railpack-{language} {receipts}/run.log', timeout=180)
        build.release()
    except Exception:
        build.diagnose()
        raise


def installed_unsupported_railpack(machine):
    build = BuildFixture(machine)
    try:
        build.start({
            'README.txt': 'Synthetic source with no supported project entrypoint.\n',
            'Dockerfile': 'FROM scratch\nLABEL ing.anima.qualification=no-fallback\n',
        }, 512)
        build.wait(360)
        build.request('refused', 'build-reconcile',
                      '.value.phase == "failed" and .value.strategy == "railpack" and .value.result == null',
                      attempt=build.attempt)
        build.request('progress', 'build-status',
                      '.value.workerProgress.stage == "detecting" and .value.workerProgress.failed',
                      attempt=build.attempt)
        console_build_diagnostics(machine, build.attempt,
                                  '.program == "railpack" and .stage == "detecting" and .finished and '
                                  '(.stdout | contains("Railpack could not determine how to build the app."))')
        refuse_export(machine, build)
        build.release()
    except Exception:
        build.diagnose()
        raise


def refuse_export(machine, build):
    payload = shlex.quote(json.dumps(dict(
        operation='build-export', attempt=build.attempt,
        archive_digest='sha256:'+'a'*64, manifest_digest='sha256:'+'b'*64,
    )))
    rejected = f'{build.receipts}/rejected.oci'
    machine.command(f'test ! -e /persist/anima-builds/{build.attempt}/work/image.oci; '
                    f'test ! -e /persist/anima-admission/reservations/{build.attempt}.json; '
                    f"if printf '%s\\n' {payload} | /run/current-system/sw/bin/anima-os-execution > {rejected}; "
                    f'then exit 1; fi; test ! -s {rejected}')

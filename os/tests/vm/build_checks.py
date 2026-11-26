"""Qualify the shipped rootless builder in a disposable, offline installed guest."""
import json
from functools import partial
import shlex
import uuid
from container_checks import wait_workspace_image


def installed_build(machine):
    """Use the authenticated root shell; all source content is synthetic."""
    workspace, context, attempt = (str(uuid.uuid4()) for _ in range(3))
    receipts = f'/run/anima-build-check-{attempt}'
    unit = f'anima-build-{attempt}.service'
    machine.command(f'mkdir -m 700 {receipts}')
    machine.command("timeout 180 sh -c 'until systemctl is-active --quiet anima-execution-recovery; do sleep 2; done'", timeout=190)
    wait_workspace_image(machine)

    request = partial(build_request, machine, receipts)

    try:
        request('prepare', 'prepare', '.value.phase == "prepared"',
                workspace=workspace, storage_bytes=1024**3)
        project = f'/persist/anima-workspaces/homes/{workspace}/workspace'
        command = ('test -r /proc/self/status && ! ( : > /proc/sysrq-trigger ) && '
                   '! ( : > /proc/sys/kernel/hostname ) && '
                   'test ! -e /run/docker.sock && test ! -e /dev/vda && '
                   'printf anima-build-stage-ok > /built')
        offline_source(machine, project, attempt, command)
        request('capture', 'capture', '.value.phase == "captured" and (.value.digest | startswith("sha256:"))',
                workspace=workspace, generation=0, context=context)
        build = dict(attempt=attempt, workspace=workspace, generation=0, context=context,
                     strategy='dockerfile',
                     resources={'memory_bytes': 512*1024**2, 'cpu_millis': 500, 'tasks': 128})
        request('launch', 'build', '.value.phase == "running" and .value.requested_strategy == "dockerfile"', **build)
        conflicting_strategy(machine, receipts, build)
        machine.command(f"timeout 300 sh -c 'while systemctl is-active --quiet {unit}; do sleep 2; done'", timeout=310)
        request('verified', 'build-reconcile',
                '.value.phase == "verified" and .value.result.integrityVerified and (.value.result.deployed | not)',
                attempt=attempt)
        machine.command(f'jq -e --slurpfile context {receipts}/capture.json '
                        f"'.value.result.sourceDigest == $context[0].value.digest' {receipts}/verified.json")
        build_diagnostics(machine, receipts, attempt)
        console_build_diagnostics(machine, attempt)
        exported_build(machine, receipts)
        request('release', 'build-release', '.value.phase == "deleted"', attempt=attempt, confirmed=True)
        machine.command(f'test ! -e /persist/anima-builds/{attempt}/work')
        released_diagnostics(machine, receipts, attempt)
        request('context-release', 'context-release', '.value.phase == "deleted"',
                workspace=workspace, generation=0, context=context, confirmed=True)
        request('workspace-release', 'workspace-release',
                '.value.phase == "deleted" and .value.storage_bytes == 0',
                workspace=workspace, generation=0, confirmed=True)
    except Exception:
        # Only this synthetic attempt is inspected; no provider or application logs.
        try:
            machine.command(f'journalctl -b -u {unit} --no-pager -n 40')
        except Exception:
            pass
        raise


def exported_build(machine, receipts):
    """The consumer checks the exact archive; a mismatched claim yields no bytes."""
    source = f'{receipts}/verified.json'
    request = f'{receipts}/export.json'
    archive = f'{receipts}/image.oci'
    machine.command(f"jq -c '{{operation:\"build-export\",attempt:.value.id,"
                    f"archive_digest:.value.result.archiveDigest,manifest_digest:.value.result.manifestDigest}}' "
                    f'{source} > {request}')
    machine.command(f'/run/current-system/sw/bin/anima-os-execution < {request} > {archive}', timeout=180)
    machine.command(f"digest=$(jq -er '.value.result.archiveDigest | ltrimstr(\"sha256:\")' {source}) || exit 1; "
                    f"printf '%s  %s\\n' \"$digest\" {archive} | sha256sum -c - || exit 1; "
                    f"test \"$(stat -c %s {archive})\" = \"$(jq -er '.value.result.bytes' {source})\"")
    exported_provenance(machine, receipts, archive)
    machine.command(f"if jq '.archive_digest = \"sha256:wrong\"' {request} | "
                    f'/run/current-system/sw/bin/anima-os-execution > {receipts}/rejected.oci; '
                    f'then exit 1; fi; test ! -s {receipts}/rejected.oci')


def conflicting_strategy(machine, receipts, build):
    payload = shlex.quote(json.dumps({'operation': 'build', **build, 'strategy': 'railpack'}))
    target = f'{receipts}/conflicting-strategy.json'
    machine.command(f"if printf '%s\\n' {payload} | /run/current-system/sw/bin/anima-os-execution > {target}; "
                    f"then exit 1; fi; jq -e '.ok == false' {target}")


def exported_provenance(machine, receipts, archive):
    """Inspect embedded provenance without extracting an archive onto the guest."""
    machine.command(
        f"digest=$(jq -er '.value.result.manifestDigest' {receipts}/verified.json) || exit 1; "
        f'tar -xOf {archive} "blobs/sha256/${{digest#sha256:}}" > {receipts}/index.json || exit 1; '
        f"jq -er '.manifests[] | select(.annotations[\"vnd.docker.reference.type\"] == "
        f"\"attestation-manifest\") | [.digest, .annotations[\"vnd.docker.reference.digest\"]] | @tsv' "
        f'{receipts}/index.json > {receipts}/attestations.tsv || exit 1; '
        f'test -s {receipts}/attestations.tsv')
    machine.command(
        f'while read -r manifest subject; do '
        f'tar -xOf {archive} "blobs/sha256/${{manifest#sha256:}}" > {receipts}/attestation.json || exit 1; '
        f"layer=$(jq -er '.layers[] | select(.mediaType == \"application/vnd.in-toto+json\") | "
        f"select(.annotations[\"in-toto.io/predicate-type\"] == \"https://slsa.dev/provenance/v0.2\") | .digest' "
        f'{receipts}/attestation.json) || exit 1; '
        f'tar -xOf {archive} "blobs/sha256/${{layer#sha256:}}" > {receipts}/provenance.json || exit 1; '
        f"jq -e --arg subject \"${{subject#sha256:}}\" "
        f"'.predicateType == \"https://slsa.dev/provenance/v0.2\" and "
        f"any(.subject[]; .digest.sha256 == $subject) and "
        f"((.predicate.materials // []) | type == \"array\")' "
        f'{receipts}/provenance.json || exit 1; '
        f'jq -e --arg layer "$layer" --slurpfile statement {receipts}/provenance.json '
        f"'.value.result.provenance as $e | "
        f"(($e.boundStatements | index($layer)) != null) and "
        f"(([($statement[0].predicate.materials // [])[] | "
        f"select(.uri | startswith(\"pkg:docker/\")) | \"sha256:\" + .digest.sha256] "
        f"- $e.baseImageDigests | length) == 0)' {receipts}/verified.json || exit 1; "
        f'done < {receipts}/attestations.tsv', timeout=90)


def build_request(machine, receipts, name, operation, assertion, **fields):
    payload = shlex.quote(json.dumps({'operation': operation, **fields}))
    target = f'{receipts}/{name}.json'
    machine.command(
        f"printf '%s\\n' {payload} | /run/current-system/sw/bin/anima-os-execution > {target} || "
        f"{{ cat {target}; exit 1; }}; cat {target}; "
        f"jq -e {shlex.quote('.ok and (' + assertion + ')')} {target}", timeout=90)


def build_diagnostics(machine, receipts, attempt):
    """Read the actual completed worker output through the root owner API."""
    build_request(machine, receipts, 'diagnostics', 'build-logs',
                  '.value.attempt == ' + json.dumps(attempt) +
                  ' and .value.program == "buildctl" and .value.stage == "building"'
                  ' and .value.finished == true'
                  ' and (.value.stdout | utf8bytelength) <= 65536'
                  ' and (.value.stderr | utf8bytelength) <= 65536', attempt=attempt)


def console_build_diagnostics(machine, attempt,
                              assertion='.program == "buildctl" and .stage == "building"'):
    """Check that the installed console can request the retained worker output."""
    filter_expression = f'any(.[]; .id == "build-logs" and .ok and (.value | .attempt == $attempt and ({assertion})))'
    machine.command(f"""jq -cn --arg attempt {shlex.quote(attempt)} '{{id:"build-logs",op:"workload-build-logs",args:{{attempt:$attempt}}}}' |
        anima-console-privileged |
        jq -se --arg attempt {shlex.quote(attempt)} {shlex.quote(filter_expression)}
    """)


def released_diagnostics(machine, receipts, attempt):
    """A released build must not disclose its former diagnostics."""
    payload = shlex.quote(json.dumps({'operation': 'build-logs', 'attempt': attempt}))
    target = f'{receipts}/released-diagnostics.json'
    machine.command(f"if printf '%s\\n' {payload} | /run/current-system/sw/bin/anima-os-execution > {target}; "
                    f"then exit 1; fi; jq -e '.ok == false and (has(\"value\") | not)' {target}")


def offline_source(machine, project, identity, command):
    """Materialize the shipped smoke rootfs as an offline Dockerfile context."""
    machine.command('docker load -i /etc/anima-smoke-image.tar.gz', timeout=120)
    name = f'anima-build-base-{identity}'
    machine.command(f'docker create --network none --name {name} anima-preview-smoke:local /bin/true >/dev/null')
    try:
        machine.command(f'docker export -o {project}/rootfs.tar {name}', timeout=120)
    finally:
        machine.command(f'docker rm {name}')
    dockerfile = 'FROM scratch\nADD rootfs.tar /\nRUN ' + json.dumps(['/bin/sh', '-c', command]) + '\n'
    machine.command(f"printf '%s' {shlex.quote(dockerfile)} > {project}/Dockerfile; chown -R 1000:1000 {project}")

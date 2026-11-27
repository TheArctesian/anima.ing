"""Check storage mounts and reporting inside a disposable VM."""
import base64
import re
import shutil
from pathlib import Path


ABSENT_VOLUME_UUID = 'e7b4c51e-d8bc-4306-9b32-2ec9b8027faa'


def storage_reporting(machine, output):
    script = base64.b64encode(Path(__file__).with_name('storage_probe.py').read_bytes()).decode()
    machine.command('test ! -e /var/lib/anima-daemon/daemon.json')
    machine.command(f"printf '%s' '{script}' | base64 -d > /tmp/anima-storage-probe.py")
    machine.command("python=$(nix-store -qR /run/current-system | grep -E '/[^/]+-python3-[0-9.]+$' | head -n1); systemd-run --unit=anima-storage-fixture --uid=anima-daemon --collect $python/bin/python3 /tmp/anima-storage-probe.py; timeout 15 sh -c 'until test -S /var/lib/anima-daemon/link.sock; do sleep 1; done'")
    try:
        machine.command("printf 'anima_enrol_AAAAAAAAAAAAAAAAAAAAAA\\n' | runuser -u anima-daemon -- env ANIMA_DAEMON_HOME=/var/lib/anima-daemon anima-daemon enrol - --hub unix:/var/lib/anima-daemon/link.sock; systemctl start anima-daemon")
        machine.command("timeout 120 sh -c 'until test -s /var/lib/anima-daemon/fixture-pushes.jsonl && jq -e -s \"any(.[]; (.drives // [] | length) > 0) and any(.[]; any(.series[]; .kind == \\\"disk_percent\\\"))\" /var/lib/anima-daemon/fixture-pushes.jsonl >/dev/null; do sleep 2; done'", timeout=125)
        start = machine.offset
        machine.command("jq -s '[.[] | {drives,series}]' /var/lib/anima-daemon/fixture-pushes.jsonl")
        (output/'storage-wire.txt').write_text(machine.buffer[start:machine.offset])
    finally:
        machine.command('systemctl stop anima-daemon anima-storage-fixture; rm -f /var/lib/anima-daemon/daemon.json /var/lib/anima-daemon/token /var/lib/anima-daemon/link.sock /var/lib/anima-daemon/fixture-pushes.jsonl /tmp/anima-storage-probe.py')


def seed_data_volume(machine):
    """Write the marker before the installed generation first boots."""
    machine.command(r"""target=$(mktemp -d /run/anima-data-fixture.XXXXXX)
        mount -t btrfs -o nosuid,nodev /dev/vdc1 "$target"
        trap 'umount "$target"' EXIT
        printf '%s\n' volume-retained > "$target/sentinel"
        sync -f "$target"
        umount "$target"
        trap - EXIT
        rmdir "$target"
    """, timeout=45)


def check_data_volume(machine):
    """Require the legacy record, actual filesystem identity and saved contents."""
    machine.command("""timeout 45 sh -c 'until systemctl is-active --quiet anima-data-volumes.service; do
          if systemctl is-failed --quiet anima-data-volumes.service; then exit 1; fi
          sleep 1
        done'
        volume=$(jq -er 'select(length == 1 and (.[0] | keys == ["model","serial","uuid"])) | .[0].uuid' /persist/data-volumes.json)
        target="/persist/volumes/$volume"
        test "$(blkid -s UUID -o value /dev/vdc1)" = "$volume"
        test "$(findmnt -nr -o UUID --mountpoint "$target")" = "$volume"
        grep -qx volume-retained "$target/sentinel"
        findmnt -nr -o TARGET,UUID --mountpoint "$target"
    """, timeout=55)


def check_capacity_frame(frame, total, used, available):
    from gui_actions import label_bounds, rendered_lines

    capacity = f'{available:.1f} GiB available of {total:.1f} GiB'
    usage = f'{used:.1f} GiB used'
    labels = ('System storage', 'Data volumes', 'Attached fixture volume', capacity, usage,
              'Absent fixture volume', 'Recorded filesystem is unmounted', 'Refresh data volumes')
    lines = rendered_lines(frame)
    bounds = [label_bounds(lines, label) for label in labels]
    system, data, attached, free, consumed, missing, unmounted, refresh = (box[1] for box in bounds)
    attached_end = max(top + height for _, top, _, height in bounds[2:5])
    assert system < data < min(attached, free, consumed), 'The attached capacity must appear under Data volumes'
    assert attached_end < min(missing, unmounted) and max(missing, unmounted) < refresh, (
        'The missing volume must appear on its own row below the attached volume')
    assert not any('gib' in word['text'].lower() for line in lines for word in line
                   if attached_end < int(word['top'])//2 < refresh), 'The absent volume must show no capacity'


def check_storage_view(machine, output, password):
    """Compare the rendered capacities to the attached disk and a missing record."""
    from console_checks import click, wait_label

    machine.send('sudo -k -i')
    machine.expect('password for admin:')
    machine.send(password)
    machine.shell('root')
    try:
        start = machine.offset
        machine.command(f'absent={ABSENT_VOLUME_UUID}; ' + r"""record=/persist/data-volumes.json
          test ! -e "/dev/disk/by-uuid/$absent"
          volume=$(jq -er '.[0].uuid' "$record")
          target="/persist/volumes/$volume"
          test "$(findmnt -nr -o UUID --mountpoint "$target")" = "$volume"
          system_mount=$(findmnt -nr -o ID --mountpoint /persist)
          volume_mount=$(findmnt -nr -o ID --mountpoint "$target")
          test "$system_mount" != "$volume_mount"
          read -r total used available < <(df -B1 --output=size,used,avail -- "$target" | tail -n 1)
          missing_target="/persist/volumes/$absent"
          cp -p "$record" /run/anima-volume-record-fixture
          mkdir "$missing_target"
          test "$(findmnt -nr -o ID --target "$missing_target")" = "$system_mount"
          jq --arg uuid "$absent" '.[0].model = "Attached fixture volume" |
            . + [{uuid:$uuid,model:"Absent fixture volume",serial:null}]' "$record" > /run/anima-volume-list-fixture
          install -m0600 /run/anima-volume-list-fixture "$record"
          jq -cn '{id:"capacity",op:"data-volumes",args:{}}' |
          anima-console-privileged |
          jq -se --arg id "$volume" --arg absent "$absent" --arg path "$target" --arg missing "$missing_target" \
            --argjson total "$total" --argjson used "$used" --argjson available "$available" '
            any(.[]; .id == "capacity" and .ok and (.value | length == 2) and
              any(.value[]; .uuid == $id and .mountpoint == $path and .state == "mounted" and
                .capacity == {total:$total,used:$used,available:$available}) and
              any(.value[]; .uuid == $absent and .mountpoint == $missing and
                .state == "not-mounted" and (has("capacity") | not)))'
          printf '%s\n' "$system_mount" > /run/anima-system-mount-fixture
          printf 'ANIMA_DATA_CAPACITY:%s:%s:%s\n' "$total" "$used" "$available"
        """, timeout=35)
        match = re.search(r'(?:^|\n)ANIMA_DATA_CAPACITY:(\d+):(\d+):(\d+)', machine.buffer[start:machine.offset])
        assert match, 'The attached volume capacity was not sampled'
        total, used, available = map(int, match.groups())
        x, y, w, h = wait_label(output, 'Storage')
        click(machine, x+w//2, y+h//2)
        wait_label(output, 'Recorded filesystem is unmounted')
        frame = output/'console-storage-capacity.ppm'
        shutil.copyfile(output/'current.ppm', frame)
        check_capacity_frame(frame, total / 1024**3, used / 1024**3, available / 1024**3)
        machine.command('test "$(findmnt -nr -o ID --mountpoint /persist)" = "$(cat /run/anima-system-mount-fixture)"')
    finally:
        machine.command('if test -f /run/anima-volume-record-fixture; then '
                        'cp -p /run/anima-volume-record-fixture /persist/data-volumes.json; '
                        f'target=/persist/volumes/{ABSENT_VOLUME_UUID}; '
                        'if test -d "$target"; then rmdir "$target"; fi; '
                        'rm -f /run/anima-volume-record-fixture /run/anima-volume-list-fixture '
                        '/run/anima-system-mount-fixture; fi')
        machine.send('exit')
        machine.shell('admin')
    return {'total': total, 'used': used, 'available': available}

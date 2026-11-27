"""Exercise the installed Docker runtime inside a disposable offline guest."""


def installed_docker(machine):
    """Called from the guest's authenticated root shell, never on the host."""
    machine.command("""timeout 180 sh -c 'until systemctl is-active --quiet docker.service; do
          if systemctl is-failed --quiet docker.service; then exit 1; fi
          sleep 1
        done' || { journalctl -b -u docker.service --no-pager -n 25; exit 1; }""", timeout=190)
    machine.command('docker load -i /etc/anima-smoke-image.tar.gz', timeout=120)
    limits = '--pull=never --network=none --read-only --memory=64m --cpus=1 --pids-limit=32 --cap-drop=ALL --security-opt=no-new-privileges'
    health = "--health-cmd 'test -f /scratch/healthy' --health-interval=1s --health-retries=1"
    labels = '--label com.docker.compose.project=fixture-health --label com.docker.compose.service=web'
    machine.command(f'docker run -d --name anima-docker-smoke {limits} {health} {labels} '
                    '--mount type=tmpfs,destination=/scratch anima-preview-smoke:local '
                    '/bin/httpd -f -p 8080 -h /www', timeout=60)
    try:
        machine.command("docker inspect --format '{{.State.Pid}}' anima-docker-smoke > /run/anima-smoke-pid; pid=$(cat /run/anima-smoke-pid); test \"$pid\" -gt 0; grep -q '^0::/anima.slice/anima-hosting.slice/' /proc/$pid/cgroup")
        machine.command("findmnt -no OPTIONS /sys/fs/cgroup | tr ',' '\\n' | grep -qx memory_recursiveprot")
        machine.command('parent=$(cat /sys/fs/cgroup/anima.slice/memory.low); '
                        'hosting=$(cat /sys/fs/cgroup/anima.slice/anima-hosting.slice/memory.low); '
                        'test "$hosting" -gt 0 && test "$parent" -ge "$hosting"')
        machine.command('docker exec anima-docker-smoke /bin/wget -qO- http://127.0.0.1:8080 | grep -qx anima-app-ok')
        docker_health(machine, 'unhealthy')
        docker_details(machine)
        machine.command('docker exec anima-docker-smoke /bin/touch /scratch/healthy')
        docker_health(machine, 'healthy')
        wait_workspace_image(machine)
        machine.command('systemctl restart docker', timeout=120)
        machine.command("docker inspect --format '{{.State.Pid}}' anima-docker-smoke | cmp /run/anima-smoke-pid -; docker exec anima-docker-smoke /bin/wget -qO- http://127.0.0.1:8080 | grep -qx anima-app-ok")
        machine.command(f'docker run --rm {limits} anima-preview-smoke:local /bin/sh -c "! touch /write-test && echo anima-job-ok" | grep -qx anima-job-ok', timeout=60)
    finally:
        machine.command('docker rm -f anima-docker-smoke', timeout=60)
    docker_log_snapshot(machine, limits)


def wait_workspace_image(machine):
    machine.command(
        """timeout 1860 sh -c 'until systemctl is-active --quiet anima-workspace-image.service; do
          if ! systemctl list-jobs --no-legend --plain | grep -q " anima-workspace-image.service "; then
            systemctl is-active --quiet anima-workspace-image.service
            exit $?
          fi
          sleep 2
        done' || { journalctl -b """
        "-u anima-workspace-image.service -u anima-workspace-storage.service "
        "-u docker.service --no-pager -n 60; exit 1; }",
        timeout=1870,
    )


def docker_health(machine, expected):
    machine.command(
        """timeout 40 sh -c 'until [ "$(docker inspect --format "{{.State.Health.Status}}" anima-docker-smoke)" = "$1" ]; do
          sleep 1
        done' sh """
        + expected
        + """
        id=$(docker inspect --format '{{.Id}}' anima-docker-smoke)
        jq -cn '{id:"health",op:"workloads",args:{}}' |
        anima-console-privileged |
        jq -se --arg id "$id" --arg health """
        + expected
        + """ 'any(.[]; .id == "health" and .ok and
          any(.value.containers[]; .id == $id and .health == $health and
            .state == "running" and .project == "fixture-health" and .service == "web"))'
        """,
        timeout=60,
    )


def docker_details(machine):
    machine.command(
        """id=$(docker inspect --format '{{.Id}}' anima-docker-smoke)
        jq -cn --arg id "$id" '{id:"details",op:"workload-details",args:{container:$id}}' |
        anima-console-privileged |
        jq -se --arg id "$id" 'any(.[]; .id == "details" and .ok and .value.container == $id and
          .value.name == "anima-docker-smoke" and .value.state == "running" and
          .value.health == "unhealthy" and .value.networks == ["none"] and
          .value.mounts == [{kind:"tmpfs",name:"",destination:"/scratch",writable:true}] and
          .value.resources.limits == {cpuMillis:1000,memoryBytes:67108864,tasks:32,cpuSet:null} and
          (.value.resources.usage | (.cpuPercent | type == "number") and (.tasks > 0) and
           (.memory | type == "string")) and
          (.value | has("workspace") | not))'""", timeout=25)


def docker_log_snapshot(machine, limits):
    machine.command(f"""docker run -d --name anima-log-smoke {limits} anima-preview-smoke:local \
        /bin/sh -c 'printf "fixture-older\\nfixture-newest\\n"'""")
    try:
        machine.command("""test "$(docker wait anima-log-smoke)" = 0
          id=$(docker inspect --format '{{.Id}}' anima-log-smoke)
          jq -cn --arg id "$id" '{id:"logs",op:"workload-logs",args:{container:$id,lines:1}}' |
          anima-console-privileged |
          jq -se --arg id "$id" 'any(.[]; .id == "logs" and .ok and .value.container == $id and
            .value.tail == 1 and (.value.stdout | endswith("fixture-newest")) and
            (.value.stdout | contains("fixture-older") | not))'""", timeout=25)
    finally:
        machine.command('docker rm -f anima-log-smoke', timeout=60)


def enable_installed_hosting(machine):
    """Prove first-run settings survive a refused service start and can be reused."""
    settings = '/persist/anima-applications/settings.json'
    machine.command(f'test ! -e {settings} && ! systemctl is-active anima-applications.service')
    hosting_status(machine, "not-configured", "inactive", False)
    machine.command(f'! anima-enable-hosting && test ! -e {settings}')
    overrides = '/run/systemd/system/anima-applications.service.d'
    dropin = f'{overrides}/50-qualification-failure.conf'
    machine.command(f'install -d {overrides}')
    machine.command(f"printf '[Service]\\nExecStartPre=/run/current-system/sw/bin/false\\n' > {dropin}")
    machine.command('systemctl daemon-reload')
    try:
        machine.command('! anima-enable-hosting --enable >/run/anima-enable-hosting.log 2>&1')
        machine.command('grep -F "sudo systemctl start anima-applications.service" /run/anima-enable-hosting.log')
        machine.command('systemctl show -p Result --value anima-applications.service | grep -qx exit-code')
        machine.command(f"stat -c '%u:%a' {settings} | grep -qx 0:600")
        machine.command(f'cp {settings} /run/hosting-settings-before-retry.json')
        hosting_status(machine, "settings-saved", "failed", False)
    finally:
        machine.command(f'rm {dropin}')
        machine.command('systemctl daemon-reload')
    machine.command('! anima-enable-hosting --enable')
    machine.command('! systemctl is-active anima-applications.service')
    machine.command('systemctl start anima-applications.service')
    machine.command('timeout 30 sh -c \'until test -S /run/anima-applications/control.sock; do sleep 1; done\'')
    machine.command('systemctl is-active anima-applications.service')
    hosting_status(machine, "settings-saved", "active", True)
    machine.command(f'cmp /run/hosting-settings-before-retry.json {settings}')
    machine.command(f"jq -e '.manageApplications.docker == \"/run/current-system/sw/bin/docker\" and "
                    f".manageApplications.compose == \"/run/current-system/sw/bin/docker-compose\"' {settings}")

def hosting_status(machine, setup, service, socket):
    machine.command(f"""jq -cn '{{id:"hosting",op:"workloads",args:{{}}}}' |
        anima-console-privileged |
        jq -se --arg setup {setup} --arg service {service} --argjson socket {str(socket).lower()} '
          any(.[]; .id == "hosting" and .ok and
            .value.hosting == {{setup:$setup,service:$service,socket:$socket}})'""", timeout=35)

{ pkgs, workspaceRoot }:
let
  keys = pkgs.runCommand "anima-update-synthetic-test-key" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out $out/private.pem 2>/dev/null
    openssl pkey -in $out/private.pem -pubout -out $out/public.pem
  '';
  package = import ../../packages/update-package.nix { inherit pkgs workspaceRoot; publicKey = "${keys}/public.pem"; };
  candidate = pkgs.runCommand "nixos-system-anima-TestCandidate" {} ''
    mkdir -p $out/etc
    echo diagnostic-fixture > $out/etc/anima-preview
    echo fixture > $out/kernel
    echo fixture > $out/initrd
    echo fixture > $out/init
    echo quiet > $out/kernel-params
  '';
  holdReview = pkgs.writeShellScript "hold-remote-update-review" ''
    ${pkgs.coreutils}/bin/touch /run/remote-review-worker-entered
    for attempt in $(${pkgs.coreutils}/bin/seq 1 300); do
      if test -e /run/remote-review-worker-released; then exit 0; fi
      ${pkgs.coreutils}/bin/sleep 0.1
    done
    exit 1
  '';
  server = pkgs.writeText "anima-update-test-server.py" ''
    import http.server, ssl, os
    os.chdir('/run/update-origin')
    class Server(http.server.SimpleHTTPRequestHandler):
      def log_message(self, *args): pass
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 443), Server)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain('/run/update-origin/certificate.pem', '/run/update-origin/tls.pem')
    server.socket = context.wrap_socket(server.socket, server_side=True)
    server.serve_forever()
  '';
in pkgs.testers.runNixOSTest {
  name = "anima-rust-update";
  nodes.machine = { lib, ... }: {
    imports = [ ../../modules/updates.nix ];
    services.anima-os-updates.package = package;
    networking.hostName = lib.mkForce "anima";
    networking.hosts."127.0.0.1" = [ "releases.anima.ing" ];
    virtualisation = { memorySize = 2048; diskSize = 8192; useBootLoader = true; useEFIBoot = true; additionalPaths = [ candidate keys server holdReview ]; docker.enable = true; };
    systemd.tmpfiles.rules = [
      "d /run/anima-os 0700 root root -"
      "d /persist/anima-admission/reservations 0700 root root -"
    ];
    environment.etc."anima/docker-client/config.json".text = "{}";
    boot.loader.systemd-boot.enable = true;
    boot.loader.efi.efiSysMountPoint = "/boot";
    boot.loader.grub.enable = false;
    users.users.observer.isNormalUser = true;
    users.users.admin.isNormalUser = true;
    environment.systemPackages = [ pkgs.python3 pkgs.openssl pkgs.jq pkgs.util-linux pkgs.nix pkgs.xz pkgs.dosfstools pkgs.cloud-utils pkgs.e2fsprogs ];
  };
  testScript = ''
    import json
    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("docker.service")
    machine.succeed("systemctl is-active anima-update-check.timer")
    machine.succeed("systemctl stop anima-update-check.timer")
    machine.succeed("mkdir -p /run/update-origin/os/preview/x86_64-linux/20261121000000 /persist/anima-updates")
    machine.succeed("nix-store --export ${candidate} > /run/system.nar; xz -1 -k /run/system.nar; cp /run/system.nar.xz /run/update-origin/os/preview/x86_64-linux/20261121000000/system.nar.xz")
    machine.succeed("openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=AnimaTestCA -addext basicConstraints=critical,CA:TRUE -keyout /run/update-origin/ca-key.pem -out /run/update-origin/ca.pem 2>/dev/null")
    machine.succeed("openssl req -new -newkey rsa:2048 -nodes -subj /CN=releases.anima.ing -addext subjectAltName=DNS:releases.anima.ing -addext basicConstraints=critical,CA:FALSE -keyout /run/update-origin/tls.pem -out /run/update-origin/request.pem 2>/dev/null")
    machine.succeed("openssl x509 -req -in /run/update-origin/request.pem -CA /run/update-origin/ca.pem -CAkey /run/update-origin/ca-key.pem -CAcreateserial -days 1 -copy_extensions copy -out /run/update-origin/certificate.pem 2>/dev/null")
    machine.succeed("systemd-run --unit=update-fixture ${pkgs.python3}/bin/python3 ${server}")
    machine.wait_for_open_port(443, timeout=15)
    script = "import json,hashlib,pathlib; p=pathlib.Path('/run/update-origin/os/preview/x86_64-linux/20261121000000/system.nar.xz'); r=dict(format=1,version='20261121000000',arch='x86_64-linux',channel='preview',system='${candidate}',sha256=hashlib.sha256(p.read_bytes()).hexdigest(),bytes=p.stat().st_size,unpackedBytes=pathlib.Path('/run/system.nar').stat().st_size,url='https://releases.anima.ing/os/preview/x86_64-linux/20261121000000/system.nar.xz'); pathlib.Path('/run/payload.json').write_text(json.dumps(r))"
    import shlex
    machine.succeed("python3 -c " + shlex.quote(script))
    machine.succeed("openssl dgst -sha256 -sign ${keys}/private.pem -out /run/signature /run/payload.json")
    envelope = "import json,base64,pathlib; e={k:base64.b64encode(pathlib.Path(p).read_bytes()).decode() for k,p in [('payload','/run/payload.json'),('signature','/run/signature')]}; pathlib.Path('/run/update-origin/os/preview/x86_64-linux/latest.json').write_text(json.dumps(e))"
    machine.succeed("python3 -c " + shlex.quote(envelope))
    prefix = "SSL_CERT_FILE=/run/update-origin/ca.pem anima-os-update "
    checked = json.loads(machine.succeed(prefix + "check"))
    assert checked["ok"] and checked["value"]["available"]
    machine.fail("su -s /bin/sh observer -c 'anima-os-update download'")
    machine.succeed("flock /run/anima-update.lock sleep 30 >/dev/null 2>&1 &")
    machine.wait_until_fails(prefix + "check")
    assert json.loads(machine.succeed(prefix + "status"))["ok"]
    machine.succeed("pkill -f '^sleep 30$'")
    machine.wait_until_succeeds(prefix + "check")
    machine.succeed("cp -a /boot /run/boot-copy; umount /boot; truncate -s 1G /run/update-esp.img; mkfs.vfat /run/update-esp.img >/dev/null; mount -o loop /run/update-esp.img /boot; cp -a /run/boot-copy/. /boot/")
    machine.succeed("growpart /dev/vda 2; resize2fs /dev/vda2; df -B1 /nix /boot")
    artifact = "/run/update-origin/os/preview/x86_64-linux/20261121000000/system.nar.xz"
    corrupt = "import pathlib; p=pathlib.Path('" + artifact + "'); b=bytearray(p.read_bytes()); b[-1]^=1; p.write_bytes(b)"
    machine.succeed("python3 -c " + shlex.quote(corrupt))
    machine.fail(prefix + "download")
    machine.succeed("test ! -e /boot/loader/entries/anima-candidate+2.conf; cp /run/system.nar.xz " + artifact)
    machine.succeed("mkdir -p /run/systemd/system/anima-update-check.service.d")
    machine.succeed("printf '[Service]\nEnvironment=SSL_CERT_FILE=/run/update-origin/ca.pem\n' > /run/systemd/system/anima-update-check.service.d/test.conf; systemctl daemon-reload")
    machine.succeed("mkdir -p /run/systemd/system/anima-update-remote-download.service.d")
    machine.succeed("cp /run/systemd/system/anima-update-check.service.d/test.conf /run/systemd/system/anima-update-remote-download.service.d/test.conf; systemctl daemon-reload")
    remote = "su -s /bin/sh admin -c 'sudo -n /run/current-system/sw/bin/anima-os-update remote-"
    machine.fail("su -s /bin/sh observer -c 'sudo -n /run/current-system/sw/bin/anima-os-update remote-status'")
    machine.fail(remote + "download-run'")
    machine.fail(remote + "download extra'")
    assert json.loads(machine.succeed(remote + "download'"))["value"]["requested"]
    machine.wait_until_succeeds("test -f /persist/anima-updates/state.json && jq -e '.phase == \"staged\"' /persist/anima-updates/state.json", timeout=120)
    remote_state = json.loads(machine.succeed(remote + "status'"))["value"]
    assert remote_state["phase"] == "staged"
    assert remote_state["system"] == "${candidate}"
    machine.wait_until_fails("systemctl is-active --quiet anima-update-remote-download.service")
    machine.succeed("rm /persist/anima-updates/state.json /boot/loader/entries/anima-candidate+2.conf")
    machine.succeed("systemctl start anima-update-check.service", timeout=120)
    staged = json.loads(machine.succeed(prefix + "status"))
    assert staged["ok"] and staged["value"]["phase"] == "staged"
    assert staged["value"]["system"] == "${candidate}"
    machine.succeed("test -f /boot/loader/entries/anima-candidate+2.conf; test -f /boot/loader/entries/anima-current.conf; test ! -e /persist/anima-updates/download.nar.xz")
    machine.succeed("sha256sum /boot/loader/loader.conf /boot/loader/entries/anima-candidate+2.conf /boot/loader/entries/anima-current.conf >/run/boot-before-cutover.sha256")
    reviews = staged["value"]["review"]
    review = shlex.quote(reviews["activate"])
    machine.succeed("cp /persist/anima-updates/state.json /run/reviewed-update.json")
    replacement = "import json,pathlib; p=pathlib.Path('/persist/anima-updates/state.json'); s=json.loads(p.read_text()); s.update(system=s['previous'],previous=s['system'],version='20261122000000'); p.write_text(json.dumps(s))"
    machine.succeed("python3 -c " + shlex.quote(replacement))
    for action in ("activate", "rollback"):
        stale = json.loads(machine.fail(prefix + action + " --review " + shlex.quote(reviews[action])))
        assert not stale["ok"] and "The update changed." in stale["error"]
    machine.succeed("sha256sum -c /run/boot-before-cutover.sha256; test ! -e /run/anima-os/maintenance.json")
    machine.succeed("cp /run/reviewed-update.json /persist/anima-updates/state.json")
    other_action = json.loads(machine.fail(prefix + "rollback --review " + review))
    assert not other_action["ok"] and "The update changed." in other_action["error"]
    remote_review = json.loads(machine.succeed(remote + "status'"))["value"]["review"]
    assert remote_review == reviews
    machine.fail(remote + "activate'")
    machine.fail(remote + "activate --review malformed'")
    machine.fail(remote + "activate-run --review " + review + "'")
    machine.fail(remote + "activate --review " + review + " extra'")
    stale_enqueue = json.loads(machine.fail(remote + "rollback --review " + review + "'"))
    assert stale_enqueue["code"] == "REVIEW_CHANGED"
    override = "/run/systemd/system/anima-update-remote-activate@.service.d"
    machine.succeed("mkdir -p " + override + "; printf '[Service]\nExecStartPre=${holdReview}\n' >" + override + "/review-gate.conf; systemctl daemon-reload")
    queued = json.loads(machine.succeed(remote + "activate --review " + review + "'"))
    assert queued["ok"] and queued["value"]["requested"]
    machine.wait_until_succeeds("test -f /run/remote-review-worker-entered")
    machine.succeed("python3 -c " + shlex.quote(replacement))
    machine.succeed("touch /run/remote-review-worker-released")
    unit = "anima-update-remote-activate@" + reviews["activate"] + ".service"
    machine.wait_until_succeeds("systemctl is-failed --quiet " + unit)
    rejected = json.loads(machine.succeed(remote + "status'"))["value"]["remote"]["result"]
    assert rejected["action"] == "activate" and "The update changed." in rejected["error"]
    machine.succeed("sha256sum -c /run/boot-before-cutover.sha256; test ! -e /run/anima-os/maintenance.json")
    machine.succeed("cp /run/reviewed-update.json /persist/anima-updates/state.json")

    candidate_entry = "/boot/loader/entries/anima-candidate+2.conf"
    machine.succeed("cp " + candidate_entry + " /run/reviewed-candidate.conf")
    different_target = "sed -i 's|init=${candidate}/init|init=" + staged["value"]["previous"] + "/init|' "
    machine.succeed(different_target + candidate_entry)
    wrong_entry = json.loads(machine.fail(prefix + "activate --review " + review))
    assert not wrong_entry["ok"] and "does not match the intended generation" in wrong_entry["error"]
    machine.succeed("test ! -e /run/anima-os/maintenance.json; cp /run/reviewed-candidate.conf " + candidate_entry + "; sha256sum -c /run/boot-before-cutover.sha256")
    cutover = "/persist/anima-applications/cutovers/0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e.json"
    machine.succeed("install -d -m 0700 /persist/anima-applications/cutovers")
    machine.succeed("umask 077; printf '%s' '{\"schema\":1,\"application\":\"0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e\"}' >" + cutover)
    denied = json.loads(machine.fail(prefix + "activate --review " + review))
    assert not denied["ok"] and "cannot honor retained Hosting cutovers" in denied["error"]
    machine.succeed("sha256sum -c /run/boot-before-cutover.sha256; test ! -e /run/anima-os/maintenance.json")
    machine.succeed("rm " + cutover)
    status = json.loads(machine.succeed(prefix + "status"))
    assert status["value"]["phase"] == "staged" and "operation" not in status["value"]
    machine.succeed("cp /persist/anima-updates/state.json /run/staged-state.json; touch /persist/anima-admission/reservations/active")
    trial = "import json,pathlib; p=pathlib.Path('/persist/anima-updates/state.json'); s=json.loads(p.read_text()); s.update(phase='booting',system=str(pathlib.Path('/run/current-system').resolve())); p.write_text(json.dumps(s))"
    machine.succeed("python3 -c " + shlex.quote(trial))
    retry = json.loads(machine.fail(prefix + "failure"))
    assert not retry["ok"] and "reservations are active" in retry["error"]
    machine.succeed("test ! -e /run/anima-os/maintenance.json; mv /run/staged-state.json /persist/anima-updates/state.json; rm /persist/anima-admission/reservations/active")

    def activation_window():
        return json.loads(machine.succeed(prefix + "status"))["value"]["activationWindow"]

    machine.succeed("systemctl is-active --quiet anima-update-window.timer; systemctl stop anima-update-window.timer")
    now = int(machine.succeed("date +%s").strip())
    start, end = now + 120, now + 360
    schedule = f"schedule-activation {review} {start} {end}"
    window = json.loads(machine.succeed(prefix + schedule))["value"]
    assert window["phase"] == "scheduled" and window["version"] == staged["value"]["version"]
    machine.succeed("systemctl start anima-update-window.service")
    assert activation_window()["id"] == window["id"] and activation_window()["phase"] == "scheduled"
    machine.fail(prefix + "cancel-activation 00000000-0000-0000-0000-000000000000")
    machine.succeed(prefix + "cancel-activation " + window["id"])
    assert activation_window()["phase"] == "cancelled"
    second = json.loads(machine.succeed(prefix + schedule))["value"]
    assert second["id"] != window["id"]
    machine.succeed("python3 -c " + shlex.quote(replacement))
    machine.succeed("systemctl start anima-update-window.service")
    assert activation_window()["phase"] == "changed"
    machine.succeed("cp /run/reviewed-update.json /persist/anima-updates/state.json")
    machine.succeed(prefix + schedule)
    machine.succeed("touch /persist/anima-admission/reservations/active")
    machine.succeed(f"date -s '@{start + 5}'; systemctl start anima-update-window.service")
    blocked = activation_window()
    assert blocked["phase"] == "deferred" and "reservations are active" in blocked["reason"]
    machine.succeed(f"date -s '@{end + 5}'; rm /persist/anima-admission/reservations/active; systemctl start anima-update-window.service")
    assert activation_window()["phase"] == "expired"
    machine.succeed("sha256sum -c /run/boot-before-cutover.sha256; test ! -e /run/anima-os/maintenance.json")
    machine.succeed("printf invalid > /persist/anima-updates/activation-window.json")
    window_unavailable = json.loads(machine.succeed(prefix + "status"))["value"]
    assert window_unavailable["activationWindow"]["phase"] == "unavailable"
    assert window_unavailable["review"]["activate"] == review
    machine.fail("systemctl start anima-update-window.service")
    machine.succeed("rm /persist/anima-updates/activation-window.json")

    machine.fail(prefix + "download")
    machine.succeed("systemctl stop update-fixture.service")
    pending = json.loads(machine.succeed(prefix + "automatic"))
    assert pending["value"]["phase"] == "staged"
    machine.succeed("systemd-run --unit=update-fixture ${pkgs.python3}/bin/python3 ${server}")
    machine.wait_for_open_port(443, timeout=15)
    machine.succeed("printf '{\"phase\":\"downloading\",\"bytes\":10,\"totalBytes\":100}' > /persist/anima-updates/progress.json")
    assert json.loads(machine.succeed(prefix + "status"))["value"]["operation"]["phase"] == "interrupted"
    machine.succeed("test $(readlink -f /run/current-system) != '${candidate}'")
    interrupted = "import json,pathlib; p=pathlib.Path('/persist/anima-updates/state.json'); s=json.loads(p.read_text()); s['phase']='booting'; p.write_text(json.dumps(s))"
    machine.succeed("python3 -c " + shlex.quote(interrupted))
    recovered = json.loads(machine.succeed(prefix + "health"))["value"]
    assert recovered["phase"] == "rolled-back" and recovered["highest"] == "20261121000000"
    held = json.loads(machine.succeed(prefix + "automatic"))["value"]
    assert held["phase"] == "held" and held["reason"] == "previous-rollback"
    assert json.loads(machine.succeed(prefix + "status"))["value"]["phase"] == "rolled-back"
    older = "import json,pathlib; p=pathlib.Path('/run/payload.json'); r=json.loads(p.read_text()); r['version']='20261120000000'; r['url']='https://releases.anima.ing/os/preview/x86_64-linux/20261120000000/system.nar.xz'; p.write_text(json.dumps(r))"
    machine.succeed("python3 -c " + shlex.quote(older))
    machine.succeed("openssl dgst -sha256 -sign ${keys}/private.pem -out /run/signature /run/payload.json")
    machine.succeed("python3 -c " + shlex.quote(envelope))
    machine.fail(prefix + "check")
  '';
}

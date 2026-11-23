{ pkgs, workspaceRoot }:
let
  keys = pkgs.runCommand "anima-update-synthetic-test-key" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out $out/private.pem 2>/dev/null
    openssl pkey -in $out/private.pem -pubout -out $out/public.pem
  '';
  package = import ./update-package.nix { inherit pkgs workspaceRoot; publicKey = "${keys}/public.pem"; };
  candidate = pkgs.runCommand "nixos-system-anima-TestCandidate" {} ''
    mkdir -p $out/etc
    echo diagnostic-fixture > $out/etc/anima-preview
    echo fixture > $out/kernel
    echo fixture > $out/initrd
    echo fixture > $out/init
    echo quiet > $out/kernel-params
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
    imports = [ ./updates.nix ];
    services.anima-os-updates.package = package;
    networking.hostName = lib.mkForce "anima";
    networking.hosts."127.0.0.1" = [ "releases.anima.ing" ];
    virtualisation = { memorySize = 2048; diskSize = 8192; useBootLoader = true; useEFIBoot = true; additionalPaths = [ candidate keys server ]; };
    boot.loader.systemd-boot.enable = true;
    boot.loader.efi.efiSysMountPoint = "/boot";
    boot.loader.grub.enable = false;
    users.users.observer.isNormalUser = true;
    environment.systemPackages = [ pkgs.python3 pkgs.openssl pkgs.jq pkgs.util-linux pkgs.nix pkgs.xz pkgs.dosfstools pkgs.cloud-utils pkgs.e2fsprogs ];
  };
  testScript = ''
    import json
    machine.start()
    machine.wait_for_unit("multi-user.target")
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
    machine.succeed("systemctl start anima-update-check.service", timeout=120)
    staged = json.loads(machine.succeed(prefix + "status"))
    assert staged["ok"] and staged["value"]["phase"] == "staged"
    assert staged["value"]["system"] == "${candidate}"
    machine.succeed("test -f /boot/loader/entries/anima-candidate+2.conf; test -f /boot/loader/entries/anima-current.conf; test ! -e /persist/anima-updates/download.nar.xz")
    status = json.loads(machine.succeed(prefix + "status"))
    assert status["value"]["phase"] == "staged" and "operation" not in status["value"]
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

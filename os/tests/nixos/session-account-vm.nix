{ pkgs, workspaceRoot }:
let
  package = import ../../packages/session-package.nix { inherit pkgs workspaceRoot; };
  server = ../session/account-server.py;
  check = ../session/account-check.py;
  link = pkgs.writeShellScriptBin "anima-link" ''
    set -eu
    test "$#" -eq 3
    test "$1" = join
    test "$2" = -state
    test "$3" = /var/lib/anima-link
    cat > /var/lib/anima-link/invite
    echo '{}' > /var/lib/anima-link/device.json
  '';
  daemon = pkgs.writeShellScriptBin "anima-daemon" (builtins.readFile ../session/account-daemon.sh);
  monitoring = pkgs.writeShellScript "account-fixture-monitoring" ''
    printf 'started\n' >> /run/account-fixture/monitoring
  '';
  certificates = pkgs.writeShellScript "account-certificates" ''
    set -eu
    cd /run/account-fixture
    ${pkgs.openssl}/bin/openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 1 -subj /CN=Synthetic-Test-CA
    ${pkgs.openssl}/bin/openssl req -newkey rsa:2048 -nodes -keyout leaf.key -out leaf.csr -subj /CN=Synthetic-Test-Service
    echo 'subjectAltName=DNS:api.workos.com,DNS:cloud.anima.ing' > extensions
    ${pkgs.openssl}/bin/openssl x509 -req -in leaf.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out leaf.pem -days 1 -extfile extensions
  '';
in pkgs.testers.runNixOSTest {
  name = "anima-os-session-account";
  nodes.machine = {
    networking.hosts."127.0.0.1" = [ "api.workos.com" "cloud.anima.ing" ];
    users.users.anima-daemon = { isSystemUser = true; group = "anima-daemon"; };
    users.groups.anima-daemon = {};
    environment.systemPackages = [ package link daemon pkgs.util-linux pkgs.coreutils pkgs.python3 ];
    systemd.tmpfiles.rules = [
      "d /run/account-fixture 0700 root root -"
      "d /persist/anima-console-activity 0700 root root -"
      "d /var/lib/anima-daemon 0700 anima-daemon anima-daemon -"
      "d /var/lib/anima-link 0700 anima-daemon anima-daemon -"
    ];
    systemd.services.account-certificates = {
      wantedBy = [ "multi-user.target" ];
      before = [ "account-https.service" ];
      serviceConfig = { Type = "oneshot"; RemainAfterExit = true; ExecStart = certificates; };
    };
    systemd.services.account-https = {
      wantedBy = [ "multi-user.target" ];
      requires = [ "account-certificates.service" ];
      after = [ "account-certificates.service" ];
      serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${server} https";
    };
    systemd.services.anima-link.serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${server} unix";
    systemd.services.anima-daemon.serviceConfig = { Type = "oneshot"; ExecStart = monitoring; };
    virtualisation.memorySize = 1024;
  };
  testScript = ''
    machine.start()
    machine.wait_for_unit("account-https.service")
    machine.wait_for_open_port(443)
    machine.succeed("SSL_CERT_FILE=/run/account-fixture/ca.pem ${pkgs.python3}/bin/python3 ${check} ${package}/bin/anima-os-session", timeout=90)
  '';
}

{ config, lib, pkgs, ... }:
let
  desktop = pkgs.writeShellScript "anima-remote-desktop" ''
    ${pkgs.openbox}/bin/openbox &
    exec /run/current-system/sw/bin/anima-console
  '';
in {
  services.openssh = {
    enable = true;
    openFirewall = false;
    listenAddresses = [ { addr = "127.0.0.1"; } { addr = "::1"; } ];
    hostKeys = [ { path = "/persist/ssh/ssh_host_ed25519_key"; type = "ed25519"; } ];
    settings = {
      PermitRootLogin = "no";
      AllowUsers = [ "admin" ];
      PasswordAuthentication = true;
      KbdInteractiveAuthentication = false;
    };
  };
  services.xrdp = {
    enable = true;
    openFirewall = false;
    sslCert = "/persist/rdp/cert.pem";
    sslKey = "/persist/rdp/key.pem";
    defaultWindowManager = "exec ${pkgs.dbus}/bin/dbus-run-session ${desktop}";
    extraConfDirCommands = ''
      substituteInPlace $out/xrdp.ini \
        --replace-fail 'port=3389' 'port=tcp://127.0.0.1:3389' \
        --replace-fail 'security_layer=negotiate' 'security_layer=tls'
      substituteInPlace $out/sesman.ini \
        --replace-fail 'AllowRootLogin=true' 'AllowRootLogin=false' \
        --replace-fail 'AlwaysGroupCheck=false' 'AlwaysGroupCheck=true' \
        --replace-fail 'TerminalServerUsers=tsusers' 'TerminalServerUsers=wheel'
    '';
  };
  # The module's --port argument overrides the loopback address in xrdp.ini.
  systemd.services.xrdp.serviceConfig.ExecStart = lib.mkForce
    "${config.services.xrdp.package}/bin/xrdp --nodaemon --config ${config.services.xrdp.confDir}/xrdp.ini";
  security.pam.services.xrdp-sesman.allowNullPassword = lib.mkForce false;
  systemd.tmpfiles.rules = [
    "d /persist/ssh 0700 root root -"
    "d /persist/rdp 0750 root xrdp -"
    # Create defaults once; a locally disabled port stays disabled across reboot/update.
    "f /var/lib/anima-link/open-ports 0600 anima-daemon anima-daemon - [22,3389]"
  ];
}

{ config, lib, pkgs, workspaceRoot ? ../.., ... }:
let
  session = import ../packages/session-package.nix { inherit pkgs workspaceRoot; };

in {
  environment.etc."anima/remote-access.json".text = builtins.toJSON {
    sshUser = "admin";
    rdpUser = "admin";
    vncUser = "admin";
  };
  systemd.services.anima-daemon.after = [ "sshd.service" "xrdp.service" "anima-vnc.service" ];
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
    defaultWindowManager = "exit 1";
    extraConfDirCommands = ''
      substituteInPlace $out/xrdp.ini \
        --replace-fail 'port=3389' 'port=tcp://127.0.0.1:3389' \
        --replace-fail 'security_layer=negotiate' 'security_layer=tls'
      # Retain global/logging/channel settings, then replace every desktop target.
      sed -i '/^\[Xorg\]/,$d' $out/xrdp.ini
      cat >> $out/xrdp.ini <<'CONSOLE'
      [Anima]
      name=Anima shared console
      lib=libvnc.so
      username=ask
      password=ask
      pamusername=same
      pampassword=same
      ip=127.0.0.1
      port=/run/anima-console/framebuffer.sock
      disabled_encodings_mask=1
      CONSOLE
      substituteInPlace $out/sesman.ini \
        --replace-fail 'AllowRootLogin=true' 'AllowRootLogin=false' \
        --replace-fail 'AlwaysGroupCheck=false' 'AlwaysGroupCheck=true' \
        --replace-fail 'TerminalServerUsers=tsusers' 'TerminalServerUsers=wheel'
    '';
  };
  # The module's --port argument overrides the loopback address in xrdp.ini.
  systemd.services.xrdp.serviceConfig.ExecStart = lib.mkForce
    "${config.services.xrdp.package}/bin/xrdp --nodaemon --config ${config.services.xrdp.confDir}/xrdp.ini";
  systemd.services.xrdp.serviceConfig.KillMode = lib.mkForce "control-group";
  systemd.services.xrdp.after = [ "anima-shared-session.service" ];
  systemd.services.xrdp.requires = [ "anima-shared-session.service" ];
  security.pam.services.xrdp-sesman.allowNullPassword = lib.mkForce false;
  systemd.services.anima-remote-access = {
    description = "Initialize private remote access defaults";
    wantedBy = [ "multi-user.target" ];
    after = [ "systemd-tmpfiles-setup.service" ];
    before = [ "anima-link.service" ];
    unitConfig.RequiresMountsFor = "/persist /var/lib/anima-link";
    serviceConfig = {
      Type = "oneshot";
      UMask = "0077";
      StateDirectory = "anima-remote-defaults";
      StateDirectoryMode = "0700";
    };
    script = "${session}/bin/anima-os-session remote-access-init";
  };
  systemd.services.anima-link = {
    requires = [ "anima-remote-access.service" ];
    after = [ "anima-remote-access.service" ];
  };
  systemd.tmpfiles.rules = [
    "d /persist/ssh 0700 root root -"
    "d /persist/rdp 0750 root xrdp -"
  ];
}

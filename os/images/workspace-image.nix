{ pkgs, daemonPackage }:
let
  railpack = import ../packages/railpack-package.nix { inherit pkgs; };
  tools = with pkgs; [ bashInteractive coreutils findutils gnugrep gnused gawk gitMinimal openssh cacert curl jq ripgrep procps socat railpack codex claude-code nodejs python3 gnutar gzip ];
  tree = pkgs.buildEnv {
    name = "anima-workspace-root";
    paths = [ daemonPackage ] ++ tools;
    pathsToLink = [ "/bin" "/etc/ssl/certs" ];
  };
  tag = builtins.substring 0 24 (builtins.hashString "sha256" (toString tree));
in pkgs.dockerTools.buildLayeredImage {
  name = "anima-workspace";
  inherit tag;
  contents = [ tree ];
  extraCommands = ''
    mkdir -p etc home/agent/workspace tmp run usr/bin
    ln -s /bin/env usr/bin/env
    chmod 1777 tmp
    printf 'root:x:0:0:root:/root:/bin/bash\nagent:x:1000:1000:Workspace:/home/agent:/bin/bash\n' > etc/passwd
    printf 'root:x:0:\nagent:x:1000:\n' > etc/group
    printf 'hosts: files dns\n' > etc/nsswitch.conf
  '';
  config = {
    User = "1000:1000";
    WorkingDir = "/home/agent/workspace";
    Env = [
      "HOME=/home/agent" "USER=agent" "LOGNAME=agent" "SHELL=/bin/bash" "PATH=/bin"
      "TMPDIR=/tmp" "LANG=C.UTF-8"
      "ANIMA_DAEMON_HOME=/home/agent/.config/anima-daemon"
      "ANIMA_MANAGED_OWNER_SOCKET=/run/anima/owner.sock"
      "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "DISABLE_AUTOUPDATER=1" "FORCE_AUTOUPDATE_PLUGINS=0"
    ];
    Entrypoint = [ "/bin/anima-daemon" "threads" ];
  };
}

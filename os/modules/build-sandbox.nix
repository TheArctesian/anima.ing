{ pkgs, lib, ... }:
let
  railpack = import ../packages/railpack-package.nix { inherit pkgs; };
  tools = pkgs.buildEnv {
    name = "anima-build-tools";
    paths = with pkgs; [ railpack buildkit rootlesskit slirp4netns runc bash coreutils util-linux iproute2 gitMinimal cacert ];
    pathsToLink = [ "/bin" ];
  };
  root = pkgs.runCommand "anima-build-sandbox-root" {} ''
    mkdir -p $out/{work,source,nix/store,etc/anima,proc,sys,dev,run/wrappers/bin,tmp,var/tmp,bin}
    # systemd cannot create missing bind-mount targets in an immutable store root.
    touch $out/etc/{passwd,group,subuid,subgid} $out/etc/anima/build-request.json
    chmod 1777 $out/tmp $out/var/tmp
    ln -s ${pkgs.bash}/bin/bash $out/bin/sh
    printf '127.0.0.1 localhost\n::1 localhost\n' > $out/etc/hosts
    printf 'nameserver 1.1.1.1\nnameserver 1.0.0.1\n' > $out/etc/resolv.conf
  '';
in {
  users.groups.anima-builder = {};
  users.users.anima-builder = {
    isSystemUser = true;
    group = "anima-builder";
    home = "/nonexistent";
    subUidRanges = [ { startUid = 2000000000; count = 65536; } ];
    subGidRanges = [ { startGid = 2000000000; count = 65536; } ];
  };
  security.wrappers.newuidmap = {
    source = lib.mkDefault "${pkgs.shadow}/bin/newuidmap";
    owner = "root"; group = "root"; setuid = true;
  };
  security.wrappers.newgidmap = {
    source = lib.mkDefault "${pkgs.shadow}/bin/newgidmap";
    owner = "root"; group = "root"; setuid = true;
  };
  environment.etc."anima/build-sandbox.json".text = builtins.toJSON {
    schema = 1;
    rootDirectory = toString root;
    path = "/run/wrappers/bin:${tools}/bin";
    rootlesskit = "${pkgs.rootlesskit}/bin/rootlesskit";
    certificates = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    workerUser = "anima-builder";
  };
  systemd.tmpfiles.rules = [ "d /persist/anima-builds 0700 root root -" "d /persist/anima-builds/locks 0700 root root -" ];
}

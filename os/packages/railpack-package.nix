{ pkgs }:
let
  version = "0.43.1";
  targets = {
    x86_64-linux = {
      archive = "x86_64-unknown-linux-musl";
      sha256 = "7a3849a68b2f142dcd9cfae68c91c5d5cb3bab4b1e749fc796cd9190a3811ea0";
    };
    aarch64-linux = {
      archive = "arm64-unknown-linux-musl";
      sha256 = "54b50c7ee1cbf2598a41db1e60652d66f9cd19864580a10457dcb533bb848d5a";
    };
  };
  target = targets.${pkgs.stdenv.hostPlatform.system};
in pkgs.stdenvNoCC.mkDerivation {
  pname = "railpack";
  inherit version;
  src = pkgs.fetchurl {
    url = "https://github.com/railwayapp/railpack/releases/download/v${version}/railpack-v${version}-${target.archive}.tar.gz";
    inherit (target) sha256;
  };
  dontUnpack = true;
  dontStrip = true;
  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin $out/share/licenses/railpack
    tar -xzf $src -C $out/bin railpack
    tar -xzf $src -C $out/share/licenses/railpack LICENSE
    chmod 0555 $out/bin/railpack
    runHook postInstall
  '';
  passthru.frontend = "ghcr.io/railwayapp/railpack-frontend@sha256:1f0bb3015946a77d33555756abdceb31885025763c63363ccfb27d83ad919ff9";
  meta = {
    description = "Pinned Railpack source build planner";
    homepage = "https://railpack.com";
    license = pkgs.lib.licenses.mit;
    platforms = builtins.attrNames targets;
    mainProgram = "railpack";
  };
}

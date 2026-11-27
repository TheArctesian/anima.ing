{ pkgs }:
pkgs.stdenv.mkDerivation {
  pname = "anima-link-preview";
  version = "0.1.0";
  src = ../payload/anima-link;
  dontUnpack = true;
  dontBuild = true;
  installPhase = ''
    mkdir -p $out/bin
    install -m755 $src $out/bin/anima-link
  '';
}

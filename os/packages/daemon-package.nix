{ pkgs }:
pkgs.stdenv.mkDerivation {
  pname = "anima-daemon-preview";
  version = "0.1.0";
  src = ../payload/anima-daemon;
  dontUnpack = true;
  nativeBuildInputs = [ pkgs.autoPatchelfHook ];
  buildInputs = [ pkgs.stdenv.cc.cc.lib ];
  dontBuild = true;
  installPhase = ''
    mkdir -p $out/bin $out/share/anima-build
    install -m755 $src $out/bin/anima-daemon
    cp ${../payload/BINARY-SHA256} $out/share/anima-build/BINARY-SHA256
  '';
  passthru.provenance = pkgs.runCommand "anima-os-source-provenance" {} ''
    mkdir -p $out/share/anima-build
    cp ${../payload/SOURCE-REVISION} $out/share/anima-build/SOURCE-REVISION
    cp ${../payload/SOURCE-SHA256} $out/share/anima-build/SOURCE-SHA256
    cp ${../payload/CARGO-LOCK-SHA256} $out/share/anima-build/CARGO-LOCK-SHA256
  '';
}

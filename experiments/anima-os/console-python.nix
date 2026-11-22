{ pkgs }:
let
  packages = pkgs.python3Packages;
  # The generator is needed to compile bindings, not to import them at runtime.
  shiboken = packages.shiboken6.overrideAttrs (old: {
    nativeBuildInputs = old.nativeBuildInputs ++ old.propagatedNativeBuildInputs;
    propagatedNativeBuildInputs = [];
  });
  pyside = (packages.pyside6.override { shiboken6 = shiboken; }).overrideAttrs (old: {
    nativeBuildInputs = old.nativeBuildInputs ++ [ packages.shiboken6-generator ];
  });
in
(pkgs.python3.withPackages (_: [ pyside ])).overrideAttrs (_: {
  disallowedRequisites = [ packages.shiboken6-generator ];
})

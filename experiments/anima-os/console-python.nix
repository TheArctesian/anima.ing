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
# Refresh transitive Python module metadata after changing Shiboken.
(pkgs.python3.withPackages (_: [ (packages.toPythonModule pyside) packages.qrcode ])).overrideAttrs (_: {
  disallowedRequisites = [ packages.shiboken6-generator ];
})

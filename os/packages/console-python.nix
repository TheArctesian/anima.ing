{ pkgs }:
let
  packages = pkgs.python3Packages;
  # The generator is needed to compile bindings, not to import them at runtime.
  shiboken = packages.shiboken6.overrideAttrs (old: {
    # Qt's regexp JIT misgenerates snippets under x86 emulation on ARM hosts.
    # Use the interpreter while generating bindings; runtime Qt is unchanged.
    QT_ENABLE_REGEXP_JIT = "0";
    nativeBuildInputs = old.nativeBuildInputs ++ old.propagatedNativeBuildInputs;
    propagatedNativeBuildInputs = [];
  });
  pyside = (packages.pyside6.override { shiboken6 = shiboken; }).overrideAttrs (old: {
    QT_ENABLE_REGEXP_JIT = "0";
    nativeBuildInputs = old.nativeBuildInputs ++ [ packages.shiboken6-generator ];
  });
in
# Refresh transitive Python module metadata after changing Shiboken.
(pkgs.python3.withPackages (_: [ (packages.toPythonModule pyside) packages.qrcode ])).overrideAttrs (_: {
  disallowedRequisites = [ packages.shiboken6-generator ];
})

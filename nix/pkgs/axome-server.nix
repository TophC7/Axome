{
  lib,
  pkgs,
  craneLib,
  version,
  workspaceSourceFilter,
  src,
  webFrontend,
}:
let
  serverArgs = {
    pname = "axome-server";
    inherit version;
    src = lib.cleanSourceWith {
      inherit src;
      filter = workspaceSourceFilter craneLib;
    };
    strictDeps = true;
    doCheck = false;
    nativeCheckInputs = [ pkgs.git ];
    preCheck = ''
      export HOME="$TMPDIR"
      export XDG_DATA_HOME="$TMPDIR/.local/share"
    '';
    cargoExtraArgs = "--locked -p axome-server";
    cargoTestExtraArgs = "-p axome-server";
  };

  serverCargoArtifacts = craneLib.buildDepsOnly (
    (builtins.removeAttrs serverArgs [
      "version"
      "nativeCheckInputs"
      "preCheck"
      "cargoTestExtraArgs"
    ])
    // {
      version =
        (builtins.fromTOML (builtins.readFile (src + "/src-crates/axome-server/Cargo.toml")))
        .package.version;
    }
  );
  unwrapped = craneLib.buildPackage (
    serverArgs
    // {
      cargoArtifacts = serverCargoArtifacts;
    }
  );
in
pkgs.runCommand "axome-server-${version}"
  {
    passthru = { inherit unwrapped; };
    meta = {
      description = "Headless Axome daemon for remote workspaces";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
      mainProgram = "axome-server";
    };
  }
  ''
    mkdir -p "$out/bin"
    cat > "$out/bin/axome-server" <<'EOF'
    #!${pkgs.runtimeShell}
    exec "${unwrapped}/bin/axome-server" --web-assets-dir "${webFrontend}" "$@"
    EOF
    chmod +x "$out/bin/axome-server"
  ''

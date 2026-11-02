{
  lib,
  pkgs,
  axome-server,
  webFrontend,
  version,
  src,
  maintainer ? "Chris Toph <toph@ryot.foo>",
  homepage ? "https://github.com/TophC7/Axome",
}:
let
  cleanVersion = builtins.replaceStrings [ "-" ] [ "." ] version;
  debArch =
    {
      "x86_64-linux" = "amd64";
      "aarch64-linux" = "arm64";
    }
    .${pkgs.stdenv.hostPlatform.system}
      or (throw "Unsupported architecture for deb: ${pkgs.stdenv.hostPlatform.system}");

  rpmArch =
    {
      "x86_64-linux" = "x86_64";
      "aarch64-linux" = "aarch64";
    }
    .${pkgs.stdenv.hostPlatform.system}
      or (throw "Unsupported architecture for rpm: ${pkgs.stdenv.hostPlatform.system}");
  tarArch =
    {
      "x86_64-linux" = "x86_64";
      "aarch64-linux" = "aarch64";
    }
    .${pkgs.stdenv.hostPlatform.system}
      or (throw "Unsupported architecture for tarball: ${pkgs.stdenv.hostPlatform.system}");

  tarBaseName = "axome-server-${cleanVersion}-linux-${tarArch}";

  # Reusable relocatable bundle containing binary, libraries, launcher and unit
  bundle = pkgs.stdenv.mkDerivation {
    pname = "axome-server-bundle";
    version = cleanVersion;

    nativeBuildInputs = [ pkgs.patchelf ];
    dontUnpack = true;
    # The launcher runs on foreign hosts; keep `#!/bin/sh` instead of a Nix store shell.
    dontPatchShebangs = true;

    buildPhase = ''
            runHook preBuild

            mkdir -p "$out/bin" "$out/lib" "$out/share/systemd/user" "$out/share/doc/axome-server" "$out/share/axome/web"

            # 1. Stripped raw binary
            cp "${axome-server.unwrapped}/bin/axome-server" "$out/bin/.axome-server-bin"
            chmod +w "$out/bin/.axome-server-bin"
            ${pkgs.stdenv.cc.targetPrefix}strip "$out/bin/.axome-server-bin"
            patchelf --set-rpath '$ORIGIN/../lib' "$out/bin/.axome-server-bin"

            # Assets compose with the binary here, without a Rust build dependency.
            cp -r "${webFrontend}/." "$out/share/axome/web/"

            # 2. Gather dynamic runtime libraries (glibc and libgcc)
            for lib in $(patchelf --print-needed "$out/bin/.axome-server-bin"); do
              found=$(find "${pkgs.glibc}/lib" "${pkgs.stdenv.cc.cc.lib}/lib" -maxdepth 1 -name "$lib" | head -n 1)
              if [ -n "$found" ]; then
                cp -L "$found" "$out/lib/"
              fi
            done
            find "${pkgs.glibc}/lib" -maxdepth 1 -name 'ld-linux*' -exec cp -L {} "$out/lib/" \;
            ldSoName="$(basename "$(find "$out/lib" -maxdepth 1 -name 'ld-linux*' | head -n 1)")"

            # 3. Wrapper script for relocatable standalone execution
            cat > "$out/bin/axome-server" <<EOF
      #!/bin/sh
      set -e
      BIN_DIR="\$(cd "\$(dirname "\$0")" && pwd)"
      BASE_DIR="\$(cd "\$BIN_DIR/.." && pwd)"
      LIB_DIR="\$BASE_DIR/lib"
      exec "\$LIB_DIR/$ldSoName" --inhibit-cache --library-path "\$LIB_DIR" --argv0 axome-server "\$BIN_DIR/.axome-server-bin" --web-assets-dir "\$BASE_DIR/share/axome/web" "\$@"
      EOF
            chmod +x "$out/bin/axome-server"

            # 4. Systemd user service
            cat > "$out/share/systemd/user/axome-server.service" <<'EOF'
      [Unit]
      Description=Axome remote workspace server
      After=network.target

      [Service]
      ExecStart=/usr/bin/axome-server serve --listen 0.0.0.0:7420
      Restart=on-failure
      KillMode=mixed

      [Install]
      WantedBy=default.target
      EOF

            # 5. Documentation and License
            install -Dm644 "${src}/LICENSE" "$out/share/doc/axome-server/LICENSE"

            runHook postBuild
    '';

    meta = {
      description = "Self-contained relocatable bundle for Axome server";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
    };
  };

  deb = pkgs.stdenv.mkDerivation {
    pname = "axome-server-deb";
    inherit version;
    nativeBuildInputs = [ pkgs.dpkg ];
    dontUnpack = true;

    buildPhase = ''
            runHook preBuild

            pkgdir="$TMPDIR/pkg"
            mkdir -p "$pkgdir/DEBIAN" "$pkgdir/usr/bin" "$pkgdir/usr/lib/axome-server/bin" "$pkgdir/usr/lib/axome-server/lib" "$pkgdir/usr/lib/systemd/user" "$pkgdir/usr/share/doc/axome-server"

            cat > "$pkgdir/DEBIAN/control" <<EOF
      Package: axome-server
      Version: ${cleanVersion}
      Section: devel
      Priority: optional
      Architecture: ${debArch}
      Maintainer: ${maintainer}
      Homepage: ${homepage}
      Depends: git
      Description: Headless Axome daemon for remote workspaces
       Axome server is a headless daemon for running coding-agent CLIs in remote workspaces.
      EOF

            mkdir -p "$pkgdir/usr/lib/axome-server/bin" "$pkgdir/usr/lib/axome-server/lib" "$pkgdir/usr/bin"
            cp -r "${bundle}/bin/." "$pkgdir/usr/lib/axome-server/bin/"
            cp -r "${bundle}/lib/." "$pkgdir/usr/lib/axome-server/lib/"
            mkdir -p "$pkgdir/usr/lib/axome-server/share/axome/web"
            cp -r "${bundle}/share/axome/web/." "$pkgdir/usr/lib/axome-server/share/axome/web/"

            cat > "$pkgdir/usr/bin/axome-server" <<'EOF'
      #!/bin/sh
      exec /usr/lib/axome-server/bin/axome-server "$@"
      EOF
            chmod +x "$pkgdir/usr/bin/axome-server"
            cp "${bundle}/share/systemd/user/axome-server.service" "$pkgdir/usr/lib/systemd/user/"
            cp "${bundle}/share/doc/axome-server/LICENSE" "$pkgdir/usr/share/doc/axome-server/"

            mkdir -p "$out"
            dpkg-deb --build --root-owner-group --threads-max=0 -Zzstd "$pkgdir" "$out/axome-server_${cleanVersion}_${debArch}.deb"

            runHook postBuild
    '';

    meta = {
      description = "Self-contained Debian package for Axome server";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
    };
  };

  rpm = pkgs.stdenv.mkDerivation {
    pname = "axome-server-rpm";
    inherit version;
    nativeBuildInputs = [ pkgs.rpm ];
    dontUnpack = true;

    buildPhase = ''
            runHook preBuild

            topdir="$TMPDIR/rpmbuild"
            mkdir -p "$topdir"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS,rpmdb}
            pkgdir="$topdir/pkg"
            mkdir -p "$pkgdir/usr/bin" "$pkgdir/usr/lib/axome-server/bin" "$pkgdir/usr/lib/axome-server/lib" "$pkgdir/usr/lib/systemd/user" "$pkgdir/usr/share/doc/axome-server"

            cp -r "${bundle}/bin/." "$pkgdir/usr/lib/axome-server/bin/"
            cp -r "${bundle}/lib/." "$pkgdir/usr/lib/axome-server/lib/"
            mkdir -p "$pkgdir/usr/lib/axome-server/share/axome/web"
            cp -r "${bundle}/share/axome/web/." "$pkgdir/usr/lib/axome-server/share/axome/web/"

            cat > "$pkgdir/usr/bin/axome-server" <<'EOF'
      #!/bin/sh
      exec /usr/lib/axome-server/bin/axome-server "$@"
      EOF
            chmod +x "$pkgdir/usr/bin/axome-server"
            cp "${bundle}/share/systemd/user/axome-server.service" "$pkgdir/usr/lib/systemd/user/"
            cp "${bundle}/share/doc/axome-server/LICENSE" "$pkgdir/usr/share/doc/axome-server/"

            cat > "$topdir/SPECS/axome-server.spec" <<EOF
      %define _binary_payload w3T.zstdio
      %define __strip /bin/true
      %define __brp_strip /bin/true
      %define __brp_ldconfig /bin/true
      %define _build_id_links none
      AutoReqProv: no

      Name: axome-server
      Version: ${cleanVersion}
      Release: 1
      Summary: Headless Axome daemon for remote workspaces
      License: AGPL-3.0-or-later
      URL: ${homepage}
      Packager: ${maintainer}
      Requires: git

      %description
      Axome server is a headless daemon for running coding-agent CLIs in remote workspaces.

      %files
      %defattr(-,root,root,-)
      /usr/bin/axome-server
      /usr/lib/axome-server
      /usr/lib/systemd/user/axome-server.service
      /usr/share/doc/axome-server/LICENSE
      EOF

            # RPM defaults to /var/tmp, which is not writable in the Nix sandbox.
            rpmbuild -bb \
              --target "${rpmArch}" \
              --define "_topdir $topdir" \
              --define "_tmppath $TMPDIR" \
              --define "_dbpath $topdir/rpmdb" \
              --buildroot "$pkgdir" \
              "$topdir/SPECS/axome-server.spec"

            mkdir -p "$out"
            cp "$topdir"/RPMS/*/*.rpm "$out/"

            runHook postBuild
    '';

    meta = {
      description = "Self-contained RPM package for Axome server";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
    };
  };

  tarball = pkgs.stdenv.mkDerivation {
    pname = "axome-server-tarball";
    inherit version;
    dontUnpack = true;

    buildPhase = ''
            runHook preBuild

            workdir="$TMPDIR/${tarBaseName}"
            mkdir -p "$workdir"
            cp -r "${bundle}/"* "$workdir/"
            ln -s share/doc/axome-server/LICENSE "$workdir/LICENSE"

            cat > "$workdir/install.sh" <<'EOF'
      #!/bin/sh
      set -e
      PREFIX="''${PREFIX:-/usr/local}"
      echo "Installing axome-server to $PREFIX..."

      SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
      BIN_DIR="$PREFIX/bin"
      LIB_DIR="$PREFIX/lib/axome-server"

      mkdir -p "$BIN_DIR" "$LIB_DIR/bin" "$LIB_DIR/lib"
      cp -r "$SCRIPT_DIR/bin/." "$LIB_DIR/bin/"
      cp -r "$SCRIPT_DIR/lib/." "$LIB_DIR/lib/"
      mkdir -p "$LIB_DIR/share/axome/web"
      cp -r "$SCRIPT_DIR/share/axome/web/." "$LIB_DIR/share/axome/web/"

      cat > "$BIN_DIR/axome-server" <<INNER
      #!/bin/sh
      exec "$LIB_DIR/bin/axome-server" "\$@"
      INNER
      chmod +x "$BIN_DIR/axome-server"

      if [ "$(id -u)" -eq 0 ] && [ "$PREFIX" = "/usr" ]; then
        SYSTEMD_DIR="/usr/lib/systemd/user"
      else
        SYSTEMD_DIR="''${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
      fi
      if mkdir -p "$SYSTEMD_DIR" 2>/dev/null && sed "s|/usr/bin/axome-server|$BIN_DIR/axome-server|g" "$SCRIPT_DIR/share/systemd/user/axome-server.service" > "$SYSTEMD_DIR/axome-server.service" 2>/dev/null; then
        echo "Installed systemd unit to $SYSTEMD_DIR/axome-server.service"
      else
        echo "Notice: skipped systemd user service installation (could not write to $SYSTEMD_DIR)"
      fi
      echo "Successfully installed axome-server to $BIN_DIR/axome-server"
      EOF
            chmod +x "$workdir/install.sh"

            mkdir -p "$out"
            tar -czf "$out/${tarBaseName}.tar.gz" -C "$TMPDIR" "${tarBaseName}"

            runHook postBuild
    '';

    meta = {
      description = "Standalone portable tarball for Axome server";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
    };
  };
in
{
  inherit
    bundle
    deb
    rpm
    tarball
    ;
}

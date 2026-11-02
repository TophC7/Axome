{
  lib,
  pkgs,
  axome,
  version ? "0.0.0",
  src,
  maintainer ? "Chris Toph <toph@ryot.foo>",
  homepage ? "https://github.com/TophC7/Axome",
}:
let
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

  # Clean version string suitable for Debian package
  debVersion = builtins.replaceStrings [ "-" ] [ "." ] version;

  # Distro loader path; the same paths exist on Fedora, Debian/Ubuntu and Arch.
  interpreter =
    {
      "x86_64-linux" = "/lib64/ld-linux-x86-64.so.2";
      "aarch64-linux" = "/lib/ld-linux-aarch64.so.1";
    }
    .${pkgs.stdenv.hostPlatform.system};

  # Distro packages run on the host's GTK/WebKitGTK and GPU drivers: a bundled
  # Nix closure cannot load host EGL vendors (EGL_BAD_PARAMETER at startup).
  installNativePayload = root: ''
    install -Dm755 ${axome}/bin/.axome-wrapped "${root}/usr/bin/axome"
    patchelf --set-interpreter ${interpreter} --remove-rpath "${root}/usr/bin/axome"
    install -Dm644 ${axome}/share/applications/axome.desktop "${root}/usr/share/applications/axome.desktop"
    sed -i 's|^Exec=axome|Exec=/usr/bin/axome|' "${root}/usr/share/applications/axome.desktop"
    mkdir -p "${root}/usr/share/icons"
    cp -r ${axome}/share/icons/. "${root}/usr/share/icons/"
    install -Dm644 ${src}/LICENSE "${root}/usr/share/doc/axome/LICENSE"
  '';

  deb = pkgs.stdenv.mkDerivation {
    pname = "axome-deb";
    inherit version;

    nativeBuildInputs = [
      pkgs.dpkg
      pkgs.gnused
      pkgs.patchelf
    ];

    dontUnpack = true;

    buildPhase = ''
            runHook preBuild

            pkgdir="$TMPDIR/pkg"
            mkdir -p "$pkgdir/DEBIAN"

            # 1. Debian control file
            cat > "$pkgdir/DEBIAN/control" <<EOF
      Package: axome
      Version: ${debVersion}
      Section: devel
      Priority: optional
      Architecture: ${debArch}
      Maintainer: ${maintainer}
      Homepage: ${homepage}
      Depends: libc6 (>= 2.39), libgtk-3-0t64 | libgtk-3-0, libwebkit2gtk-4.1-0, git
      Description: Linux-first desktop app for coding-agent CLIs
       Axome is a Linux-first desktop development environment for working with coding-agent CLIs.
      EOF

            # 2. Binary, desktop entry, icons and license
            ${installNativePayload "$pkgdir"}

            # 3. Build the deb package
            mkdir -p "$out"
            debFileName="axome_${debVersion}_${debArch}.deb"
            dpkg-deb --build --root-owner-group --threads-max=0 -Zzstd "$pkgdir" "$out/$debFileName"

            runHook postBuild
    '';

    meta = {
      description = "Debian package for Axome desktop (system GTK/WebKitGTK)";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
    };
  };

  rpm = pkgs.stdenv.mkDerivation {
    pname = "axome-rpm";
    inherit version;

    nativeBuildInputs = [
      pkgs.rpm
      pkgs.gnused
      pkgs.patchelf
    ];

    dontUnpack = true;

    buildPhase = ''
            runHook preBuild

            topdir="$TMPDIR/rpmbuild"
            mkdir -p "$topdir"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS,rpmdb}
            pkgdir="$topdir/pkg"

            # 1. Binary, desktop entry, icons and license
            ${installNativePayload "$pkgdir"}

            # 2. RPM spec file
            cat > "$topdir/SPECS/axome.spec" <<EOF
      %define _binary_payload w3T.zstdio
      %define __strip /bin/true
      %define __brp_strip /bin/true
      %define __brp_ldconfig /bin/true
      %define _build_id_links none
      AutoReqProv: no

      Name: axome
      Version: ${debVersion}
      Release: 1
      Summary: Linux-first desktop app for coding-agent CLIs
      License: AGPL-3.0-or-later
      URL: ${homepage}
      Packager: ${maintainer}
      Requires: glibc >= 2.39, gtk3, webkit2gtk4.1, git

      %description
      Axome is a Linux-first desktop development environment for working with coding-agent CLIs.

      %files
      %defattr(-,root,root,-)
      /usr/bin/axome
      /usr/share/applications/axome.desktop
      /usr/share/icons/hicolor/*/*/*
      /usr/share/doc/axome
      EOF

            # 3. Build the RPM package
            # RPM defaults to /var/tmp, which is not writable in the Nix sandbox.
            rpmbuild -bb \
              --target "${rpmArch}" \
              --define "_topdir $topdir" \
              --define "_tmppath $TMPDIR" \
              --define "_dbpath $topdir/rpmdb" \
              --buildroot "$pkgdir" \
              "$topdir/SPECS/axome.spec"

            mkdir -p "$out"
            cp "$topdir"/RPMS/*/*.rpm "$out/"

            runHook postBuild
    '';

    meta = {
      description = "RPM package for Axome desktop (system GTK/WebKitGTK)";
      license = lib.licenses.agpl3Plus;
      platforms = lib.platforms.linux;
    };
  };

  pkgbuildTemplate = ''
    pkgname=axome-bin
    pkgver=@VERSION@
    pkgrel=1
    pkgdesc='Linux-first desktop app for coding-agent CLIs'
    arch=('x86_64' 'aarch64')
    url='${homepage}'
    license=('AGPL-3.0-or-later')
    depends=(
      'gcc-libs'
      'git'
      'glib2'
      'glibc'
      'gtk3'
      'webkit2gtk-4.1'
    )
    makedepends=('libarchive')
    provides=("axome=$pkgver")
    conflicts=('axome')
    options=('!strip')
    source_x86_64=("https://github.com/TophC7/Axome/releases/download/@COMMIT@/axome_''${pkgver}_amd64.deb")
    sha256sums_x86_64=('@AMD64_SHA256@')
    source_aarch64=("https://github.com/TophC7/Axome/releases/download/@COMMIT@/axome_''${pkgver}_arm64.deb")
    sha256sums_aarch64=('@ARM64_SHA256@')
    noextract=("axome_''${pkgver}_amd64.deb" "axome_''${pkgver}_arm64.deb")

    package() {
      local deb_arch
      case "$CARCH" in
        x86_64) deb_arch=amd64 ;;
        aarch64) deb_arch=arm64 ;;
      esac

      bsdtar -xOf "$srcdir/axome_''${pkgver}_''${deb_arch}.deb" 'data.tar.*' |
        bsdtar -xf - -C "$pkgdir"

      install -Dm644 "$pkgdir/usr/share/doc/axome/LICENSE" \
        "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
      rm -f "$pkgdir/usr/share/doc/axome/LICENSE"
      rmdir --ignore-fail-on-non-empty "$pkgdir/usr/share/doc/axome" "$pkgdir/usr/share/doc"
    }
  '';

  generate-aur = pkgs.writeShellScriptBin "generate-aur" ''
        set -euo pipefail
        export PATH="${pkgs.pacman}/bin:${pkgs.coreutils}/bin:${pkgs.gnused}/bin:${pkgs.gnutar}/bin:$PATH"

        if [ "$#" -ne 2 ]; then
          echo "Usage: generate-aur <package-version> <dist-directory>" >&2
          exit 1
        fi

        RAW_VERSION="$1"
        VERSION="''${RAW_VERSION//-/.}"
        DIST_DIR="$2"
        COMMIT_HASH="''${RAW_VERSION#*+}"

        case "$RAW_VERSION" in
          *+*) ;;
          *)
            echo "Error: Version must match <version>+<commit-hash>, got: $RAW_VERSION" >&2
            exit 1
            ;;
        esac

        AMD64_DEB="$DIST_DIR/axome_''${VERSION}_amd64.deb"
        ARM64_DEB="$DIST_DIR/axome_''${VERSION}_arm64.deb"

        if [ ! -f "$AMD64_DEB" ] || [ ! -f "$ARM64_DEB" ]; then
          echo "Missing release artifacts in $DIST_DIR: $AMD64_DEB or $ARM64_DEB" >&2
          exit 1
        fi

        AMD64_SHA256=$(sha256sum "$AMD64_DEB" | cut -d' ' -f1)
        ARM64_SHA256=$(sha256sum "$ARM64_DEB" | cut -d' ' -f1)

        WORK_DIR="$DIST_DIR/aur"
        mkdir -p "$WORK_DIR"

        cat > "$WORK_DIR/PKGBUILD" <<'EOF'
    ${pkgbuildTemplate}
    EOF

        sed -i \
          -e "s|@VERSION@|$VERSION|g" \
          -e "s|@COMMIT@|$COMMIT_HASH|g" \
          -e "s|@AMD64_SHA256@|$AMD64_SHA256|g" \
          -e "s|@ARM64_SHA256@|$ARM64_SHA256|g" \
          "$WORK_DIR/PKGBUILD"

        CONF="${pkgs.pacman}/etc/makepkg.conf"

        (
          cd "$WORK_DIR"
          makepkg --config "$CONF" --printsrcinfo > .SRCINFO
        )

        tar -czf "$DIST_DIR/axome-bin-$COMMIT_HASH.tar.gz" -C "$WORK_DIR" PKGBUILD .SRCINFO
        echo "Generated $DIST_DIR/axome-bin-$COMMIT_HASH.tar.gz"
  '';
in
{
  inherit deb rpm generate-aur;
}

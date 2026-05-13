Name:           __PACKAGE_NAME__
Version:        __RPM_VERSION__
Release:        __RPM_RELEASE__%{?dist}
Summary:        Codex App for Linux
License:        Proprietary
ExclusiveArch:  __ARCH__
Provides:       codex-desktop
Obsoletes:      codex-desktop
%global __requires_exclude_from ^/opt/__PACKAGE_NAME__/.*$
%global __provides_exclude_from ^/opt/__PACKAGE_NAME__/.*$

Requires:       python3, /usr/bin/7z
__UPDATER_REQUIRES__
Requires:       alsa-lib, at-spi2-atk, atk, glib2, gtk3, libdrm
Requires:       nspr, nss, pango, libstdc++, libX11, libxcb
Requires:       libXcomposite, libXdamage, libXext, libXfixes, libxkbcommon, libXrandr
Requires:       mesa-libgbm
Recommends:     zenity, kdialog

%description
Community-built Linux package for Codex App generated from the macOS DMG.
Requires the Codex CLI to be available in PATH or CODEX_CLI_PATH.
__UPDATER_DESCRIPTION__

%install
# Files are staged by build-rpm.sh outside of BUILDROOT and copied here.
mkdir -p %{buildroot}
cp -a "__RPM_STAGING_DIR__/." "%{buildroot}/"

%files
%defattr(-,root,root,-)
/opt/__PACKAGE_NAME__/
/usr/lib/__PACKAGE_NAME__/
/usr/bin/__PACKAGE_NAME__
/usr/share/applications/__PACKAGE_NAME__.desktop
/usr/share/icons/hicolor/256x256/apps/__PACKAGE_NAME__.png
__UPDATER_FILES__

%post
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi

__UPDATER_POST__

%preun
__UPDATER_PREUN__

%postun
__UPDATER_POSTUN__

%changelog
* Thu Jan 01 2026 Codex App Linux Maintainers <maintainers@codex-app-linux>
- Initial RPM package

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
%global codex_elf_suffix %{nil}
%ifarch x86_64 aarch64 ppc64le s390x riscv64
%global codex_elf_suffix ()(64bit)
%endif

Requires:       python3
__UPDATER_REQUIRES__
Requires:       libasound.so.2%{codex_elf_suffix}, libatk-bridge-2.0.so.0%{codex_elf_suffix}
Requires:       libatk-1.0.so.0%{codex_elf_suffix}, libglib-2.0.so.0%{codex_elf_suffix}, libgtk-3.so.0%{codex_elf_suffix}
Requires:       libdrm.so.2%{codex_elf_suffix}, libnspr4.so%{codex_elf_suffix}, libnss3.so%{codex_elf_suffix}
Requires:       libpango-1.0.so.0%{codex_elf_suffix}, libstdc++.so.6%{codex_elf_suffix}, libX11.so.6%{codex_elf_suffix}
Requires:       libxcb.so.1%{codex_elf_suffix}, libXcomposite.so.1%{codex_elf_suffix}, libXdamage.so.1%{codex_elf_suffix}
Requires:       libXext.so.6%{codex_elf_suffix}, libXfixes.so.3%{codex_elf_suffix}, libxkbcommon.so.0%{codex_elf_suffix}
Requires:       libXrandr.so.2%{codex_elf_suffix}, libgbm.so.1%{codex_elf_suffix}
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

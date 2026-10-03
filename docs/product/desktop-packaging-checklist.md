# Desktop Packaging Checklist

## Store identity

Locked to the package already uploaded for this product. Do not change the name or the publisher.

- Package name: `mcp-tool-shop.CommandUI`
- Publisher display name: `mcp-tool-shop`
- Executable: `commandui-desktop.exe`
- Entry point: `Windows.FullTrustApplication`
- Architecture: x64
- Version floor: greater than `1.0.1.0`
- Upload: unsigned. Partner Center signs it.

## App identity
- [ ] Product name: CommandUI
- [ ] Bundle identifier: com.commandui.desktop
- [ ] Desktop version matches the four-part package version, with `.0` on the end
- [ ] Store logos present under `packaging/msix/Assets`

## Functional
- [ ] App launches clean
- [ ] Session creates on boot
- [ ] Commands execute in real shell
- [ ] Semantic plans generate and review
- [ ] Persistence works (restart test)

## Platform Checks
- [ ] Windows: `packaging/pack-msix.ps1` prints `RESULT PASS` and `signed no`
- [ ] Windows: MSI/NSIS builds, for the direct download
- [ ] macOS: DMG/app builds
- [ ] Linux: deb builds

## Documentation
- [ ] README updated
- [ ] Known limitations current
- [ ] Release notes drafted

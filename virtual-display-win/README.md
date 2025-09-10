# virtual-display-win

Windows virtual display helpers for VM/agent scenarios. Provides:

- Display enumeration via DisplayConfig (through GDI APIs)
- Heuristics to detect presence of an Indirect Display Driver (IddCx)
- Convenience wrappers to install/uninstall a driver via `pnputil`

Important:
- This crate does not implement a kernel-mode IddCx driver. It focuses on user-mode helpers and a minimal API.
- For a true virtual monitor, supply/install a signed IddCx-based driver package.

## Example

```bash
cargo run -p virtual-display-win --example list_displays
```

## API Sketch

- `enumerate_displays() -> Result<Vec<DisplayTargetInfo>>`
- `is_virtual_driver_present() -> Result<bool>`
- `install_virtual_driver_via_pnputil(inf_path: &str) -> Result<bool>`
- `uninstall_virtual_driver_via_pnputil(published_name: &str) -> Result<bool>`

## Integrating With Agents

- Use `is_virtual_driver_present` at startup to verify environment
- If absent (and you have admin), call `install_virtual_driver_via_pnputil` with your `.inf`
- `enumerate_displays` can validate at least one active display for UI Automation

## Notes on IddCx

- IddCx is the Windows Indirect Display Driver Class eXtension for building virtual/indirect displays.
- Implemented in kernel-mode; user-mode setup requires a signed driver package and `pnputil` or device installation APIs.

# Compatibility utility for the manual browser preview's shutdown.
# No automated test calls this script; it never focuses, resizes or reads input.
param(
    [Parameter(Mandatory)][ValidateSet('Close')][string]$Mode,
    [Parameter(Mandatory)][ValidateRange(1,2147483647)][int]$AppProcessId
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class MemMapPreviewClose {
    delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, StringBuilder name, int length);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder name, int length);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    public static void Close(uint pid) {
        EnumWindows((hwnd, data) => {
            uint owner; GetWindowThreadProcessId(hwnd, out owner);
            if (owner != pid) return true;
            var kind = new StringBuilder(256); var title = new StringBuilder(256);
            GetClassName(hwnd, kind, kind.Capacity); GetWindowText(hwnd, title, title.Capacity);
            if ((kind.ToString() == "Tauri Window" && title.ToString() == "Mem Map") || kind.ToString() == "#32770")
                PostMessage(hwnd, 16, IntPtr.Zero, IntPtr.Zero);
            return true;
        }, IntPtr.Zero);
    }
}
'@
[MemMapPreviewClose]::Close([uint32]$AppProcessId)
'{"Result":"CLOSE_REQUESTED"}'

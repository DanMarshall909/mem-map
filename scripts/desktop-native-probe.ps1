param(
    [Parameter(Mandatory)][ValidateSet('Inspect', 'Clipboard', 'ClipboardState', 'SaveCsv', 'InspectSave', 'Windows', 'Close', 'Resize', 'Csv')][string]$Mode,
    [int]$AppProcessId,
    [string]$Destination,
    [int]$Width,
    [int]$Height,
    [uint32]$AfterSequence,
    [switch]$PrepareOnly
)
$ErrorActionPreference = 'Stop'
if ($Mode -eq 'Csv') {
    @(Import-Csv -LiteralPath $Destination) | ConvertTo-Json -Compress
    exit 0
}
if ($AppProcessId -le 0) { throw 'Native probe requires the exact owned process ID.' }
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
# Framework proxy discovery requires a typed caller frame. PowerShell's dynamic
# frames can otherwise silently expose native Edit/Button HWNDs only as Panes.
Add-Type -ReferencedAssemblies ([Windows.Automation.AutomationElement].Assembly.Location) @'
public static class MemMapUIAClientBootstrap {
    [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
    public static void Initialize() {
        var proxies=typeof(System.Windows.Automation.AutomationElement).Assembly.GetName();
        proxies.Name="UIAutomationClientsideProviders";
        System.Windows.Automation.ClientSettings.RegisterClientSideProviderAssembly(proxies);
    }
}
'@
[MemMapUIAClientBootstrap]::Initialize()
Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Collections.Generic;
public static class MemMapNativeWindows {
    public delegate bool EnumProc(IntPtr hwnd,IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc,IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder name,int length);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd,StringBuilder name,int length);
    [DllImport("user32.dll",SetLastError=true)] public static extern bool SetWindowPos(IntPtr hwnd,IntPtr after,int x,int y,int width,int height,uint flags);
    [DllImport("user32.dll",SetLastError=true)] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll",SetLastError=true)] public static extern bool PostMessage(IntPtr hwnd,uint message,IntPtr wparam,IntPtr lparam);
    [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
    public static IntPtr[] ForPid(int pid) {
        var result=new List<IntPtr>();
        EnumWindows(delegate(IntPtr hwnd,IntPtr data){uint owner;GetWindowThreadProcessId(hwnd,out owner);if(owner==(uint)pid)result.Add(hwnd);return true;},IntPtr.Zero);
        return result.ToArray();
    }
    public static string Class(IntPtr hwnd){var name=new StringBuilder(256);GetClassName(hwnd,name,256);return name.ToString();}
    public static string Title(IntPtr hwnd){var name=new StringBuilder(256);GetWindowText(hwnd,name,256);return name.ToString();}
    public static Rect ResizePhysical(IntPtr hwnd,int width,int height) {
        var old=SetThreadDpiAwarenessContext(new IntPtr(-4));
        if(old==IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(),"SetThreadDpiAwarenessContext");
        try {
            if(!SetWindowPos(hwnd,IntPtr.Zero,0,0,width,height,22))throw new Win32Exception(Marshal.GetLastWin32Error(),"SetWindowPos");
            Rect bounds;if(!GetWindowRect(hwnd,out bounds))throw new Win32Exception(Marshal.GetLastWin32Error(),"GetWindowRect");return bounds;
        } finally {SetThreadDpiAwarenessContext(old);}
    }
}
public static class MemMapClipboardBoundary {
    [DllImport("user32.dll")] public static extern uint GetClipboardSequenceNumber();
    [DllImport("user32.dll")] public static extern IntPtr GetClipboardOwner();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", SetLastError=true)] static extern bool OpenClipboard(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool CloseClipboard();
    [DllImport("user32.dll", SetLastError=true)] static extern IntPtr GetClipboardData(uint format);
    [DllImport("kernel32.dll")] static extern IntPtr GlobalLock(IntPtr handle);
    [DllImport("kernel32.dll")] static extern bool GlobalUnlock(IntPtr handle);
    public static string ReadOwned(int pid, uint after) {
        if (!OpenClipboard(IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error(), "OpenClipboard");
        try {
            uint owner; var hwnd=GetClipboardOwner(); GetWindowThreadProcessId(hwnd,out owner);
            if (owner!=(uint)pid || GetClipboardSequenceNumber()==after) throw new InvalidOperationException("Clipboard is not a fresh copy owned by the test process");
            var handle=GetClipboardData(13); if(handle==IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(),"CF_UNICODETEXT");
            var pointer=GlobalLock(handle); if(pointer==IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(),"GlobalLock");
            try { return Marshal.PtrToStringUni(pointer); } finally { GlobalUnlock(handle); }
        } finally { CloseClipboard(); }
    }
}
'@
if ($Mode -eq 'ClipboardState') {
    $owner=[MemMapClipboardBoundary]::GetClipboardOwner(); [uint32]$ownerPid=0
    [void][MemMapClipboardBoundary]::GetWindowThreadProcessId($owner,[ref]$ownerPid)
    [pscustomobject]@{ Sequence=[MemMapClipboardBoundary]::GetClipboardSequenceNumber(); Owner=$owner.ToInt64(); ProcessId=$ownerPid } | ConvertTo-Json -Compress
    exit 0
}
if ($Mode -eq 'Clipboard') { [MemMapClipboardBoundary]::ReadOwned($AppProcessId,$AfterSequence) | ConvertTo-Json -Compress; exit 0 }
$deadline = [DateTime]::UtcNow.AddSeconds(20)
$pidCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $AppProcessId)
function Get-AppWindows {
    [Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children, $pidCondition)
}
if ($Mode -eq 'Windows') {
    @([MemMapNativeWindows]::ForPid($AppProcessId) | ForEach-Object { [pscustomobject]@{Hwnd=$_.ToInt64();Class=[MemMapNativeWindows]::Class($_);Title=[MemMapNativeWindows]::Title($_)} }) | ConvertTo-Json -Compress; exit 0
}
if ($Mode -eq 'Close') {
    foreach($hwnd in [MemMapNativeWindows]::ForPid($AppProcessId)) {
        if([MemMapNativeWindows]::Class($hwnd) -eq '#32770' -or ([MemMapNativeWindows]::Class($hwnd) -eq 'Tauri Window' -and [MemMapNativeWindows]::Title($hwnd) -eq 'Mem Map')) {
            [void][MemMapNativeWindows]::PostMessage($hwnd,16,[IntPtr]::Zero,[IntPtr]::Zero)
        }
    }
    '{"Result":"CLOSE_REQUESTED"}'; exit 0
}
if ($Mode -eq 'Resize') {
    $hwnd=@([MemMapNativeWindows]::ForPid($AppProcessId) | Where-Object { [MemMapNativeWindows]::Class($_) -eq 'Tauri Window' -and [MemMapNativeWindows]::Title($_) -eq 'Mem Map' })
    if($hwnd.Count -ne 1 -or $Width -lt 400 -or $Height -lt 300){throw 'Resize requires one exact owned Tauri window and bounded positive dimensions.'}
    $bounds=[MemMapNativeWindows]::ResizePhysical($hwnd[0],$Width,$Height)
    [pscustomobject]@{ Result = 'PASS'; Hwnd=$hwnd[0].ToInt64(); Width = $bounds.Right-$bounds.Left; Height = $bounds.Bottom-$bounds.Top } | ConvertTo-Json -Compress
    exit 0
}
if ($Mode -eq 'Inspect') {
    do {
        $names = @()
        $selectedRows = 0; $expandedRows = 0; $measuredRows = 0
        foreach ($window in (Get-AppWindows)) {
            $all = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
            foreach ($item in $all) {
                if ($item.Current.Name) { $names += $item.Current.Name }
                if ($item.Current.Name -like '*; Private working set *; Private commit *') {
                    $measuredRows++
                    try { if ($item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Current.IsSelected) { $selectedRows++ } } catch {}
                    try { if ($item.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern).Current.ExpandCollapseState -eq [Windows.Automation.ExpandCollapseState]::Expanded) { $expandedRows++ } } catch {}
                }
            }
        }
        if (($names -contains 'Processes') -and ($names -contains 'Pause display') -and $selectedRows -gt 0 -and $expandedRows -gt 0) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($names -notcontains 'Processes') { throw 'WebView2 UI Automation does not expose the Processes treegrid.' }
    if ($names -notcontains 'Pause display') { throw 'WebView2 UI Automation does not expose the Pause display button.' }
    if ($selectedRows -lt 1 -or $expandedRows -lt 1) { throw "UI Automation must expose selected/expanded measured process rows; selected=$selectedRows expanded=$expandedRows measured=$measuredRows" }
    [pscustomobject]@{ Result = 'PASS'; NamedElements = $names.Count; Treegrid = $true; Pause = $true; SelectedRows=$selectedRows; ExpandedRows=$expandedRows; MeasuredRows=$measuredRows } | ConvertTo-Json -Compress
    exit 0
}
if ($Mode -eq 'SaveCsv' -and (!$Destination -or ![IO.Path]::IsPathRooted($Destination) -or [IO.Path]::GetFullPath($Destination) -ne $Destination)) { throw 'Save probe requires an absolute normalized destination.' }
if ($Mode -eq 'SaveCsv' -and (Test-Path -LiteralPath $Destination)) { throw 'Save probe refuses to overwrite an existing destination.' }
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class MemMapSaveInput {
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")]
    public static extern IntPtr Text(IntPtr hwnd, uint message, IntPtr length, string text);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")]
    public static extern IntPtr Read(IntPtr hwnd, uint message, IntPtr length, StringBuilder text);
    [DllImport("user32.dll", EntryPoint="SendMessageW")]
    public static extern IntPtr Click(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
}
'@
do {
    foreach ($window in (Get-AppWindows)) {
        $dialog = if ($window.Current.ClassName -eq '#32770') { $window } else {
            $window.FindFirst([Windows.Automation.TreeScope]::Descendants,
                [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty, '#32770'))
        }
        if (!$dialog) { continue }
        if($Mode -eq 'InspectSave') {
            $items=@(); foreach($item in $dialog.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)) {
                $patterns=@($item.GetSupportedPatterns() | ForEach-Object ProgrammaticName)
                $value=$null; try {$value=$item.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value} catch {}
                $items += [pscustomobject]@{Name=$item.Current.Name;Class=$item.Current.ClassName;Id=$item.Current.AutomationId;Type=$item.Current.ControlType.ProgrammaticName;Hwnd=$item.Current.NativeWindowHandle;Value=$value;Patterns=$patterns}
            }
            [pscustomobject]@{Dialog=$dialog.Current.NativeWindowHandle;Name=$dialog.Current.Name;Items=$items} | ConvertTo-Json -Depth 8 -Compress; exit 0
        }
        $editCondition = [Windows.Automation.AndCondition]::new(
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty, 'Edit'),
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty, '1001'))
        $edit = $dialog.FindFirst([Windows.Automation.TreeScope]::Descendants, $editCondition)
        $saveCondition = [Windows.Automation.AndCondition]::new(
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty, 'Button'),
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty, '1'))
        $save = $dialog.FindFirst([Windows.Automation.TreeScope]::Descendants, $saveCondition)
        if (!$edit -or !$save) { continue }
        $filename = $dialog.FindFirst([Windows.Automation.TreeScope]::Descendants,
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty, 'FileNameControlHost'))
        if (!$filename) { throw 'Owned common item dialog has no filename model control.' }
        $edit = $filename.FindFirst([Windows.Automation.TreeScope]::Descendants, $editCondition)
        if (!$edit) { throw 'Filename model has no native edit descendant.' }
        $editHandle = [IntPtr]$edit.Current.NativeWindowHandle
        $saveHandle = [IntPtr]$save.Current.NativeWindowHandle
        if ($editHandle -eq [IntPtr]::Zero -or $saveHandle -eq [IntPtr]::Zero) { throw 'Native save controls have no HWND.' }
        [uint32]$dialogPid=0
        [void][MemMapClipboardBoundary]::GetWindowThreadProcessId([IntPtr]$dialog.Current.NativeWindowHandle,[ref]$dialogPid)
        if ($dialogPid -ne $AppProcessId) { throw 'Save dialog does not belong to the owned application.' }
        $filenamePattern=$filename.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
        if ($filenamePattern.Current.IsReadOnly) { throw 'Filename control is read-only.' }
        $edit.SetFocus()
        # Targeted ordinary character input lets the dialog receive its actual
        # edit/change notifications. A programmatic ValuePattern/WM_SETTEXT can
        # change visible text while leaving the common-dialog result unchanged.
        [void][MemMapSaveInput]::Click($editHandle, 177, [IntPtr]::Zero, [IntPtr](-1))
        foreach($character in $Destination.ToCharArray()) {
            [void][MemMapSaveInput]::Click($editHandle, 258, [IntPtr]([int]$character), [IntPtr]::Zero)
        }
        $save.SetFocus()
        # UIA SetValue may be completed by the dialog thread after it returns.
        # Do not activate Save until two independent consumers agree on the
        # complete destination. Reading this native Edit also rejects a wrong
        # similarly numbered shell/address edit selected elsewhere in the dialog.
        $stable=0
        do {
            $readback = New-Object Text.StringBuilder 32768
            [void][MemMapSaveInput]::Read($editHandle, 13, [IntPtr]32768, $readback)
            $chosen=$filenamePattern.Current.Value
            if ([string]::Equals($chosen,$Destination,[StringComparison]::OrdinalIgnoreCase) -and $readback.ToString() -eq $Destination) { $stable++ } else { $stable=0 }
            if($stable -ge 3) { break }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($stable -lt 3 -or $readback.ToString() -ne $Destination -or ![string]::Equals($chosen,$Destination,[StringComparison]::OrdinalIgnoreCase)) { throw "Native filename model/edit differ; model=[$chosen] edit=[$($readback.ToString())] wanted=[$Destination]" }
        $dialogHandle=$dialog.Current.NativeWindowHandle
        $filenameHandle=$filename.Current.NativeWindowHandle
        if (!$PrepareOnly) { $save.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke() }
        [pscustomobject]@{ Result = 'PASS'; NativeSaveDialog = $true; PreparedOnly=[bool]$PrepareOnly; DialogHwnd=$dialogHandle; ProcessId=$dialogPid; FilenameHwnd=$filenameHandle; EditHwnd=$editHandle.ToInt64(); ChosenPath=$chosen; NativeEditText=$readback.ToString(); StableReadings=$stable; InputMethod='Owned edit WM_CHAR + Save focus commit'; SaveInvoked=(!$PrepareOnly) } | ConvertTo-Json -Compress
        exit 0
    }
    Start-Sleep -Milliseconds 150
} while ([DateTime]::UtcNow -lt $deadline)
throw 'The owned application did not expose a native CSV save dialog.'

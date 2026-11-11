<#
Starts Impulse, drives it with posted window messages and captures it with PrintWindow, then kills it.
The real cursor, the foreground window and the screen are never touched, so it is safe to run while
the machine is in use. Coordinates are client pixels of the window.

  scripts\ui.ps1 -Steps 'click:1016,25;wait:800;shot:out.png'

Steps, separated by ';':  wait:<ms>  move:x,y  click:x,y  rclick:x,y  wheel:x,y,<notches>  text:<chars>  key:<virtual key>  shot:<png path>  mem:<label> (working set and its private part, committed, GPU memory, CPU seconds so far; the WebView2 bridge's processes apart)
  threads:<label> (CPU seconds since the last threads step, by thread name or the module it started in)
#>
param(
    [string]$Steps,
    [string]$Exe = (Join-Path $PSScriptRoot '..\target\debug\Impulse.exe')
)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public delegate bool Cb(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref RECT at);
  // The wheel message carries screen coordinates.
  public static IntPtr OnScreen(IntPtr h, int x, int y) {
    var at = new RECT { L = x, T = y };
    ClientToScreen(h, ref at);
    return (IntPtr)((at.T << 16) | (at.L & 0xFFFF));
  }
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] static extern bool EnumWindows(Cb cb, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("kernel32.dll")] static extern IntPtr OpenThread(uint access, bool inherit, uint id);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] static extern int GetThreadDescription(IntPtr h, out IntPtr text);
  [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr p);
  public static string ThreadName(int id) {
    IntPtr h = OpenThread(0x0800, false, (uint)id), text;
    if (h == IntPtr.Zero) return "";
    string name = "";
    if (GetThreadDescription(h, out text) >= 0 && text != IntPtr.Zero) { name = Marshal.PtrToStringUni(text); LocalFree(text); }
    CloseHandle(h);
    return name;
  }
  [DllImport("ntdll.dll")] static extern int NtQueryInformationThread(IntPtr h, int kind, out IntPtr info, int size, IntPtr written);
  public static long StartAddress(int id) {
    IntPtr h = OpenThread(0x0040, false, (uint)id), start = IntPtr.Zero;
    if (h == IntPtr.Zero) return 0;
    NtQueryInformationThread(h, 9, out start, IntPtr.Size, IntPtr.Zero);
    CloseHandle(h);
    return start.ToInt64();
  }
  // The process also owns 16 px helper windows, so the window is found by its class.
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, _) => {
      uint p; GetWindowThreadProcessId(h, out p);
      var c = new StringBuilder(64); GetClassName(h, c, 64);
      if (p == pid && c.ToString() == "Zed::Window") { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
'@
[W]::SetProcessDPIAware() | Out-Null
# A second instance would share the data directory with the first.
if (Get-Process impulse -ErrorAction SilentlyContinue) { throw 'Impulse is already running: close it first' }

$p = Start-Process -FilePath $Exe -PassThru
try {
    $h = [IntPtr]::Zero
    $r = New-Object W+RECT
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 500
        $h = [W]::Find($p.Id)
        if ($h -ne [IntPtr]::Zero -and [W]::GetClientRect($h, [ref]$r) -and ($r.R - $r.L) -gt 400) { break }
    }
    if ($r.R - $r.L -le 400) { throw 'no window' }
    # The first frame and the data load need a moment.
    Start-Sleep -Seconds 4

    function Post($message, $x, $y, $keys = 0) {
        [W]::PostMessage($h, $message, [IntPtr]$keys, [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))) | Out-Null
    }
    foreach ($step in ($Steps -split ';')) {
        $kind, $arg = $step -split ':', 2
        switch ($kind) {
            'wait' { Start-Sleep -Milliseconds ([int]$arg) }
            'move' { $x, $y = [int[]]($arg -split ','); Post 0x200 $x $y }
            'click' {
                $x, $y = [int[]]($arg -split ',')
                Post 0x200 $x $y; Start-Sleep -Milliseconds 80
                Post 0x201 $x $y 1; Start-Sleep -Milliseconds 60
                Post 0x202 $x $y; Start-Sleep -Milliseconds 400
            }
            'rclick' {
                $x, $y = [int[]]($arg -split ',')
                Post 0x200 $x $y; Start-Sleep -Milliseconds 80
                Post 0x204 $x $y 2; Start-Sleep -Milliseconds 60
                Post 0x205 $x $y; Start-Sleep -Milliseconds 400
            }
            'wheel' {
                $x, $y, $notches = [int[]]($arg -split ',')
                Post 0x200 $x $y; Start-Sleep -Milliseconds 80
                [W]::PostMessage($h, 0x20A, [IntPtr](($notches * 120) -shl 16), [W]::OnScreen($h, $x, $y)) | Out-Null
                Start-Sleep -Milliseconds 400
            }
            'key' {
                [W]::PostMessage($h, 0x100, [IntPtr][int]$arg, [IntPtr]1) | Out-Null; Start-Sleep -Milliseconds 60
                [W]::PostMessage($h, 0x101, [IntPtr][int]$arg, [IntPtr]0xC0000001L) | Out-Null; Start-Sleep -Milliseconds 400
            }
            'text' {
                foreach ($c in $arg.ToCharArray()) {
                    [W]::PostMessage($h, 0x102, [IntPtr][int]$c, [IntPtr]1) | Out-Null
                    Start-Sleep -Milliseconds 30
                }
                Start-Sleep -Milliseconds 400
            }
            'shot' {
                $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
                $g = [System.Drawing.Graphics]::FromImage($bmp)
                $dc = $g.GetHdc()
                # Client only: the whole window starts at the invisible resize border, off from click coordinates.
                [W]::PrintWindow($h, $dc, 3) | Out-Null
                $g.ReleaseHdc($dc); $g.Dispose()
                $bmp.Save($arg, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
                "saved $arg"
            }
            'mem' {
                $p.Refresh()
                # Task Manager's "Memory" is the private working set; the GPU's own memory is counted apart.
                $counters = (Get-Counter "\Process(impulse)\Working Set - Private", "\GPU Process Memory(pid_$($p.Id)_*)\Dedicated Usage", "\GPU Process Memory(pid_$($p.Id)_*)\Shared Usage" -ErrorAction SilentlyContinue).CounterSamples
                $mb = { param($name) ($counters | Where-Object Path -like "*$name" | Measure-Object CookedValue -Sum).Sum / 1MB }
                "{0}: working set {1:N0} MB (private {2:N0}), committed {3:N0} MB, gpu {4:N0} + shared {5:N0} MB, cpu {6:N2} s" -f $arg, ($p.WorkingSet64 / 1MB), (& $mb 'working set - private'), ($p.PrivateMemorySize64 / 1MB), (& $mb 'dedicated usage'), (& $mb 'shared usage'), $p.TotalProcessorTime.TotalSeconds
                # The WebView2 bridge: a browser process under Impulse and its children.
                $web = @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'")
                $ids = @($p.Id)
                foreach ($depth in 1..3) { $ids = $ids + @($web | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId) }
                $bridge = @(Get-CimInstance Win32_PerfRawData_PerfProc_Process | Where-Object { $_.IDProcess -ne $p.Id -and $ids -contains $_.IDProcess })
                "  webview2: {0} processes, private {1:N0} MB, committed {2:N0} MB" -f $bridge.Count, (($bridge | Measure-Object WorkingSetPrivate -Sum).Sum / 1MB), (($bridge | Measure-Object PrivateBytes -Sum).Sum / 1MB)
            }
            'threads' {
                $p.Refresh()
                $main = ($p.Threads | Sort-Object StartTime | Select-Object -First 1).Id
                $modules = $p.Modules
                $now = @{}
                foreach ($t in $p.Threads) { $now[$t.Id] = $t.TotalProcessorTime.TotalSeconds }
                # Unnamed threads go by the module they started in.
                $rows = foreach ($id in $now.Keys) {
                    $before = if ($seen -and $seen.ContainsKey($id)) { $seen[$id] } else { 0 }
                    $name = if ($id -eq $main) { 'main' } else { [W]::ThreadName($id) }
                    if (-not $name) {
                        $start = [W]::StartAddress($id)
                        $module = $modules | Where-Object { $start -ge $_.BaseAddress.ToInt64() -and $start -lt $_.BaseAddress.ToInt64() + $_.ModuleMemorySize } | Select-Object -First 1
                        $name = if ($module) { $module.ModuleName } else { '?' }
                    }
                    [pscustomobject]@{ cpu = $now[$id] - $before; name = $name }
                }
                $seen = $now
                "${arg}: {0:N2} s" -f ($rows | Measure-Object cpu -Sum).Sum
                $rows | Group-Object name | ForEach-Object { [pscustomobject]@{ cpu = ($_.Group | Measure-Object cpu -Sum).Sum; name = $_.Name; count = $_.Count } } |
                    Sort-Object cpu -Descending | Select-Object -First 8 | ForEach-Object { "  {0,6:N2} s  {1} x{2}" -f $_.cpu, $_.name, $_.count }
            }
            default { throw "unknown step '$step'" }
        }
    }
} finally {
    if (-not $p.HasExited) { $p.Kill() }
}

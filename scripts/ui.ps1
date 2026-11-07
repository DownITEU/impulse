<#
Starts Impulse, drives it with posted window messages and captures it with PrintWindow, then kills it.
The real cursor, the foreground window and the screen are never touched, so it is safe to run while
the machine is in use. Coordinates are client pixels of the window.

  scripts\ui.ps1 -Steps 'click:1016,25;wait:800;shot:out.png'

Steps, separated by ';':  wait:<ms>  move:x,y  click:x,y  text:<chars>  shot:<png path>
#>
param(
    [string]$Steps,
    [string]$Exe = (Join-Path $PSScriptRoot '..\target\debug\impulse.exe')
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
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] static extern bool EnumWindows(Cb cb, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
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
                [W]::PrintWindow($h, $dc, 2) | Out-Null
                $g.ReleaseHdc($dc); $g.Dispose()
                $bmp.Save($arg, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
                "saved $arg"
            }
            default { throw "unknown step '$step'" }
        }
    }
} finally {
    if (-not $p.HasExited) { $p.Kill() }
}

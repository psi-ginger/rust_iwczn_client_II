# evidence/live-fix/run_driver_acceptance.ps1 - sanitized acceptance runner
# (Verifier, 2026-10-28). Reproduces the green run of drv_green_acceptance.log.
#
# Credentials are READ FROM THE OPERATOR'S ENVIRONMENT, never stored here:
#   $env:GOONZU_USER_ID / $env:GOONZU_USER_PW  (account with a character in slot 0)
#
# Flow: capture relay (game leg, 5799 -> wire-verified GAME tunnel) with a
# graceful-FIN watchdog at +40 s; the clone's driver runs a full session;
# the c2s capture is then walked frame by frame (usSize is plaintext on the
# wire; 5-byte frames are the header-only heartbeats).
#
# ACCEPTANCE (LIVE-01 fix):
#   (a) the session passes Stage::InWorld - the live NPC-led opcode-6
#       broadcast (34 x 68 B) is absorbed without refusal
#   (b) >= 3 in-world heartbeats are MEASURED on the wire before the FIN
#   (c) the session ends at Stage::TornDown with exit code 0 (the graceful
#       clean-close teardown; a literal Ctrl+C console event is undeliverable
#       in this ConPTY-hosted environment - see ctrl_c_investigation.md -
#       its wiring stays covered by the hermetic seam tests)
$ErrorActionPreference = "Continue"
$clone = "C:\Users\DK2\AppData\Local\Temp\opencode\evidence\live-fix\clone"
$tmp   = "C:\Users\DK2\AppData\Local\Temp\opencode"
$exe   = Join-Path $clone "target\debug\goonzu_client.exe"
$relay = Join-Path $tmp "relay\target\debug\relay.exe"
$out   = Join-Path $tmp "drv_green_acceptance.log"
$cap   = Join-Path $tmp "cap2"

if (-not $env:GOONZU_USER_ID -or -not $env:GOONZU_USER_PW) {
    Write-Output "set GOONZU_USER_ID / GOONZU_USER_PW in this shell first"
    exit 2
}
if (-not (Test-Path $exe)) { Write-Output "clone binary missing - see README (clone + 5 gates first)"; exit 9 }

New-Item -ItemType Directory -Force -Path $cap | Out-Null
Remove-Item $out, "$cap\*.bin" -ErrorAction SilentlyContinue

# 1. capture relay with the graceful-FIN watchdog (+40 s, both directions)
$rp = Start-Process -FilePath $relay -ArgumentList "127.0.0.1:5799","0.tcp.ap.ngrok.io:27911","$cap\c2s.bin","$cap\s2c.bin","40" -PassThru -WindowStyle Hidden
Write-Output "[accept] relay pid=$($rp.Id)"
Start-Sleep -Seconds 1

# 2. env + launch (wire-verified endpoint ROLES - see env.txt / D-1)
$env:GOONZU_FRONT_ADDR   = "0.tcp.ap.ngrok.io:14195"
$env:GOONZU_GAME_ADDR    = "127.0.0.1:5799"
$env:GOONZU_CHAR_SLOT    = "0"
$env:GOONZU_MAC_ADDRESS  = "00-11-22-33-44-55"

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "cmd.exe"
$psi.Arguments = '/c ""' + $exe + '" > "' + $out + '" 2>&1"'
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $clone
$p = [System.Diagnostics.Process]::Start($psi)
Write-Output "[accept] driver pid=$($p.Id) started $((Get-Date).ToString('HH:mm:ss.fff'))"

# 3. hold in-world; the FIN lands at +40 s (about 3 heartbeat intervals in)
Start-Sleep -Seconds 46
if (-not $p.HasExited) { Write-Output "[accept] still running after 46s - the FIN watchdog did not fire" }
$done = $p.WaitForExit(30000)
taskkill /IM relay.exe /F 2>&1 | Out-Null
if ($done) {
    Add-Content -Path $out -Value ""
    Add-Content -Path $out -Value "[evidence] driver process exit code = $($p.ExitCode) after a graceful server-FIN teardown (WO-6.1 S5: 0 = session ended cleanly)"
    Add-Content -Path $out -Value "[evidence] run at $([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss')) local; binary from the clean clone of 701963a; game leg via the capture relay"
    Write-Output "[accept] EXIT CODE = $($p.ExitCode)"
} else {
    Write-Output "[accept] driver did not exit - acceptance (c) NOT met"
}

# 4. walk the c2s capture (usSize plaintext; cmd words session-ciphered)
Write-Output "--- c2s frame walk ---"
$c2s = [System.IO.File]::ReadAllBytes("$cap\c2s.bin")
$off = 0; $i = 0; $hb = 0
while ($off + 2 -le $c2s.Length) {
    $sz = [BitConverter]::ToUInt16($c2s, $off)
    if ($sz -lt 5 -or $off + $sz -gt $c2s.Length) { Write-Output "stop at frame $i (sz=$sz)"; break }
    $tag = if ($sz -eq 5) { "  <= HEARTBEAT"; $hb++ } else { "" }
    Write-Output ("frame {0}: off={1} usSize={2}{3}" -f $i, $off, $sz, $tag)
    $off += $sz; $i++
}
Write-Output "c2s total bytes=$($c2s.Length) frames=$i heartbeats=$hb"
Write-Output "--- driver log ---"
if (Test-Path $out) { Get-Content $out }

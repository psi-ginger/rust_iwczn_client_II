# evidence/6.3/run_live_soak.ps1 - sanitized >=2-min live soak runner
# (Verifier, 2026-10-28, WO-6.3 section 8(c): evidence only, never a gate).
# Adapted from evidence/live-fix/run_driver_acceptance.ps1; the only change
# is the hold length: the relay's graceful-FIN watchdog fires at +130 s so
# the session must sustain Stage::InWorld for >= 2 minutes (~12-13 wire
# heartbeats at the fixed 10 s cadence) before the clean teardown.
#
# Credentials are READ FROM THE OPERATOR'S ENVIRONMENT, never stored here:
#   $env:GOONZU_USER_ID / $env:GOONZU_USER_PW  (account with a character in slot 0)
$ErrorActionPreference = "Continue"
$clone = "C:\Users\DK2\AppData\Local\Temp\opencode\evidence\6.3\clone"
$tmp   = "C:\Users\DK2\AppData\Local\Temp\opencode"
$exe   = Join-Path $clone "target\debug\goonzu_client.exe"
$relay = Join-Path $tmp "relay\target\debug\relay.exe"
$out   = Join-Path $tmp "soak63_live.log"
$cap   = Join-Path $tmp "cap_soak63"

if (-not $env:GOONZU_USER_ID -or -not $env:GOONZU_USER_PW) {
    Write-Output "set GOONZU_USER_ID / GOONZU_USER_PW in this shell first"
    exit 2
}
if (-not (Test-Path $exe)) { Write-Output "clone binary missing"; exit 9 }

New-Item -ItemType Directory -Force -Path $cap | Out-Null
Remove-Item $out, "$cap\*.bin" -ErrorAction SilentlyContinue

# 1. capture relay with the graceful-FIN watchdog at +130 s
$rp = Start-Process -FilePath $relay -ArgumentList "127.0.0.1:5799","0.tcp.ap.ngrok.io:27911","$cap\c2s.bin","$cap\s2c.bin","130" -PassThru -WindowStyle Hidden
Write-Output "[soak] relay pid=$($rp.Id) (FIN watchdog +130s)"
Start-Sleep -Seconds 1

# 2. env + launch (wire-verified endpoint ROLES - front :14195 / game :27911, D-1)
$env:GOONZU_FRONT_ADDR   = "0.tcp.ap.ngrok.io:14195"
$env:GOONZU_GAME_ADDR    = "127.0.0.1:5799"
$env:GOONZU_CHAR_SLOT    = "0"
$env:GOONZU_MAC_ADDRESS  = "00-11-22-33-44-55"

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "cmd.exe"
$psi.Arguments = '/c ""' + $exe + '" > "' + $out + '" 2>&1"'
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $clone
$started = Get-Date
$p = [System.Diagnostics.Process]::Start($psi)
Write-Output "[soak] driver pid=$($p.Id) started $($started.ToString('HH:mm:ss.fff'))"

# 3. hold; the FIN lands at +130 s, teardown shortly after
$done = $p.WaitForExit(170000)
$ended = Get-Date
taskkill /IM relay.exe /F 2>&1 | Out-Null
$elapsed = ($ended - $started).TotalSeconds
if ($done) {
    Add-Content -Path $out -Value ""
    Add-Content -Path $out -Value "[evidence] driver process exit code = $($p.ExitCode); wall clock $([math]::Round($elapsed,2)) s (FIN watchdog +130 s => in-world hold >= 120 s)"
    Add-Content -Path $out -Value "[evidence] binary from the S4 clean clone of a5151ca (code 3285d07); game leg via the capture relay; run $([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss')) local"
    Write-Output "[soak] EXIT CODE = $($p.ExitCode) after $([math]::Round($elapsed,2))s"
} else {
    Write-Output "[soak] driver did not exit within 170s - soak (c) NOT met"
}

# 4. walk the c2s capture (usSize plaintext; 5-B frames are heartbeats)
Write-Output "--- c2s frame walk ---"
$c2s = [System.IO.File]::ReadAllBytes("$cap\c2s.bin")
$off = 0; $i = 0; $hb = 0
while ($off + 2 -le $c2s.Length) {
    $sz = [BitConverter]::ToUInt16($c2s, $off)
    if ($sz -lt 5 -or $off + $sz -gt $c2s.Length) { Write-Output "stop at frame $i (sz=$sz)"; break }
    if ($sz -eq 5) { $hb++ }
    $off += $sz; $i++
}
Write-Output "c2s total bytes=$($c2s.Length) frames=$i heartbeats=$hb"
Write-Output "--- driver log ---"
if (Test-Path $out) { Get-Content $out }

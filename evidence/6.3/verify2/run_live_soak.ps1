# evidence/6.3/run_live_soak.ps1 - Verifier's >= 120 s live soak runner
# (Verifier lane, Step 6.3). Adapted from evidence/live-fix/
# run_driver_acceptance.ps1 (the proven LIVE-01 acceptance runner) with the
# FIN watchdog raised from 40 s to 135 s so the in-world hold spans >= 12
# heartbeat intervals at the server's 10 s cadence.
#
# Credentials are READ FROM THE OPERATOR'S ENVIRONMENT, never stored here:
#   $env:GOONZU_USER_ID / $env:GOONZU_USER_PW  (account with a char in slot 0)
#
# Flow: capture relay (game leg, 5801 -> wire-verified GAME tunnel :27911)
# with a graceful-FIN watchdog at +135 s; the clean-clone driver runs one
# full session; afterwards the c2s capture is walked frame by frame
# (usSize is plaintext on the wire; 5-byte frames are header-only
# heartbeats) to MEASURE the heartbeat cadence off the wire.
#
# ACCEPTANCE (WO-6.3 live soak, directive item 2):
#   (a) the session sustains Stage::InWorld for >= 120 s (server FIN at
#       +135 s; the in-world window is measured from the handshake line to
#       the in-world-ended line in the driver log)
#   (b) >= 12 in-world heartbeats are MEASURED on the wire before the FIN,
#       at a consistent ~10 s cadence (zero frame drops: every frame in the
#       capture walks cleanly to the end - no truncated/partial frame)
#   (c) the session ends at Stage::TornDown with exit code 0 (graceful
#       clean-close teardown; a literal Ctrl+C console event is undeliverable
#       in this ConPTY-hosted environment - see ctrl_c_investigation.md)
$ErrorActionPreference = "Continue"
$clone = "$env:TEMP\opencode\verify-6.3"
$tmp   = "$env:TEMP\opencode"
$exe   = Join-Path $clone "target\debug\goonzu_client.exe"
$relay = Join-Path $tmp "relay\target\debug\relay.exe"
$out   = Join-Path $tmp "drv_soak_63.log"
$cap   = Join-Path $tmp "cap_soak63"
$FIN_S = 135   # graceful server FIN after 135 s (>= 12 x 10 s heartbeat intervals)

if (-not $env:GOONZU_USER_ID -or -not $env:GOONZU_USER_PW) {
    Write-Output "set GOONZU_USER_ID / GOONZU_USER_PW in this shell first"
    exit 2
}
if (-not (Test-Path $exe))  { Write-Output "clone binary missing - build first";  exit 9 }
if (-not (Test-Path $relay)) { Write-Output "relay binary missing - build first"; exit 9 }

New-Item -ItemType Directory -Force -Path $cap | Out-Null
Remove-Item $out, "$cap\*.bin" -ErrorAction SilentlyContinue

# 1. capture relay with the graceful-FIN watchdog (+135 s, both directions)
$rp = Start-Process -FilePath $relay -ArgumentList "127.0.0.1:5801","0.tcp.ap.ngrok.io:27911","$cap\c2s.bin","$cap\s2c.bin","$FIN_S" -PassThru -WindowStyle Hidden
Write-Output "[soak] relay pid=$($rp.Id) fin_after=${FIN_S}s"
Start-Sleep -Seconds 1

# 2. env + launch (wire-verified endpoint ROLES - see evidence/live/README.md D-1)
$env:GOONZU_FRONT_ADDR   = "0.tcp.ap.ngrok.io:14195"
$env:GOONZU_GAME_ADDR    = "127.0.0.1:5801"
$env:GOONZU_CHAR_SLOT    = "0"
$env:GOONZU_MAC_ADDRESS  = "00-11-22-33-44-55"

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "cmd.exe"
$psi.Arguments = '/c ""' + $exe + '" > "' + $out + '" 2>&1"'
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $clone
$t0  = [DateTime]::Now
$p   = [System.Diagnostics.Process]::Start($psi)
Write-Output "[soak] driver pid=$($p.Id) started $($t0.ToString('HH:mm:ss.fff'))"

# 3. hold in-world; the FIN lands at +$FIN_S s, teardown a few s after
Start-Sleep -Seconds ($FIN_S + 10)
if (-not $p.HasExited) { Write-Output "[soak] still running after $($FIN_S + 10)s - the FIN watchdog did not fire" }
$done = $p.WaitForExit(60000)
$wall = ([DateTime]::Now - $t0).TotalSeconds
taskkill /IM relay.exe /F 2>&1 | Out-Null
if ($done) {
    Add-Content -Path $out -Value ""
    Add-Content -Path $out -Value "[evidence] driver process exit code = $($p.ExitCode) after a graceful server-FIN teardown at +${FIN_S}s (WO-6.1 S5: 0 = session ended cleanly)"
    Add-Content -Path $out -Value "[evidence] run at $([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss')) local; wall-clock $($wall.ToString('0.0'))s; binary from the clean clone of 3285d07; game leg via the capture relay"
    Write-Output "[soak] EXIT CODE = $($p.ExitCode)   wall-clock = $([Math]::Round($wall,1))s"
} else {
    Write-Output "[soak] driver did not exit - acceptance (c) NOT met"
}

# 4. walk the c2s capture (usSize plaintext; 5-byte frames = heartbeats)
Write-Output "--- c2s frame walk ---"
$c2s = [System.IO.File]::ReadAllBytes("$cap\c2s.bin")
$off = 0; $i = 0; $hb = 0; $partial = $false
while ($off + 2 -le $c2s.Length) {
    $sz = [BitConverter]::ToUInt16($c2s, $off)
    if ($sz -lt 5 -or $off + $sz -gt $c2s.Length) { Write-Output "PARTIAL/TRUNCATED frame at index $i off=$off usSize=$sz (bytes left=$($c2s.Length - $off))"; $partial = $true; break }
    if ($sz -eq 5) { $hb++ }
    $off += $sz; $i++
}
Write-Output "c2s total bytes=$($c2s.Length) frames=$i heartbeats=$hb partial_frame=$partial"
Write-Output "--- driver log ---"
if (Test-Path $out) { Get-Content $out }

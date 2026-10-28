# evidence/live/run_capture_session.ps1 - sanitized capture-session runner (Verifier, 2026-10-28).
#
# Transparent-TCP-relay capture of one live driver session: the game leg is
# routed through evidence/live/relay (built from relay/Cargo.toml + relay/main.rs),
# which appends every raw byte in both directions to cap/c2s.bin and cap/s2c.bin
# while forwarding untouched. The front leg goes direct.
#
# The captures in cap/ came from THIS script's flow (run 2026-10-28, the
# relayed LIVE-01 reproduction, drv_live_relayed.log). The offline read-back
# of cap/s2c.bin is decode.log / decode2.log, produced by decode_tool.rs.
#
# CREDENTIALS ARE READ FROM THE OPERATOR'S ENVIRONMENT AND NEVER STORED HERE
# (see run_driver_live.ps1 for the full env contract).
$ErrorActionPreference = "Continue"
$repo = "C:\Data\Client\rust_iwczn_client_II"
$tmp  = "C:\Users\DK2\AppData\Local\Temp\opencode"
$exe  = Join-Path $repo "target\debug\goonzu_client.exe"
$out  = Join-Path $tmp "drv_live_relayed.log"

if (-not $env:GOONZU_USER_ID -or -not $env:GOONZU_USER_PW) {
    Write-Output "set GOONZU_USER_ID / GOONZU_USER_PW in this shell first (not stored in evidence)"
    exit 2
}
$env:GOONZU_FRONT_ADDR   = "0.tcp.ap.ngrok.io:14195"   # wire-verified FRONT
$env:GOONZU_GAME_ADDR    = "127.0.0.1:5799"            # the relay below
$env:GOONZU_CHAR_SLOT    = "0"
$env:GOONZU_MAC_ADDRESS  = "00-11-22-33-44-55"
Remove-Item $out -ErrorAction SilentlyContinue
Remove-Item "$tmp\cap\*.bin" -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path "$tmp\cap" | Out-Null

# 1. build the relay (std-only, no dependencies)
Push-Location (Join-Path $repo "evidence\live\relay")
cargo build --quiet 2>&1 | Write-Output
PopLocation
$relayExe = Join-Path $repo "evidence\live\relay\target\debug\relay.exe"
if (-not (Test-Path $relayExe)) { Write-Output "relay build failed"; exit 9 }

# 2. start the relay in a background job
$job = Start-Job -ScriptBlock {
    param($exe, $listen, $target, $c2s, $s2c)
    & $exe $listen $target $c2s $s2c 2>&1
} -ArgumentList $relayExe, "127.0.0.1:5799", "0.tcp.ap.ngrok.io:27911", "$tmp\cap\c2s.bin", "$tmp\cap\s2c.bin"
Start-Sleep -Seconds 2

# 3. run the driver through it (dies at the LIVE-01 refusal, ~1.1s in)
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "cmd.exe"
$psi.Arguments = '/c ""' + $exe + '" > "' + $out + '" 2>&1"'
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $repo
$p = [System.Diagnostics.Process]::Start($psi)
$p.WaitForExit()
Write-Output "[driver] EXIT CODE = $($p.ExitCode)"

# 4. stop the relay; report capture sizes
Stop-Job $job -ErrorAction SilentlyContinue
Remove-Job $job -Force -ErrorAction SilentlyContinue
Get-Content $out
Write-Output "--- capture sizes ---"
Get-ChildItem "$tmp\cap" -File | Select-Object Name, Length

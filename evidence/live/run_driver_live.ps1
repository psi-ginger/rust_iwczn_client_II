# evidence/live/run_driver_live.ps1 - sanitized reproduction runner (Verifier, 2026-10-28).
#
# Runs the full driver session against the live server and prints a
# self-contained log with the process exit code appended (the canonical
# LIVE-01 reproduction; see README.md).
#
# CREDENTIALS ARE READ FROM THE OPERATOR'S ENVIRONMENT AND NEVER STORED HERE.
# Set them before running:
#   $env:GOONZU_USER_ID   = <live account>
#   $env:GOONZU_USER_PW   = <password>
# The account must hold a character in GOONZU_CHAR_SLOT (the driver refuses
# an empty slot and has no create-char path).
#
# NOTE (D-1): the endpoint ROLES below are wire-verified, not label-verified.
# Upstream labelled 27911 "Front" and 14195 "Game"; the wire says the
# opposite (14195 pushes the plaintext opcode-30000 SECRETKEY frame,
# 27911 pushes the ciphered 13-byte CLIENTCHARUNIQUE frame).
$ErrorActionPreference = "Continue"
$repo = "C:\Data\Client\rust_iwczn_client_II"
$tmp  = "C:\Users\DK2\AppData\Local\Temp\opencode"
$exe  = Join-Path $repo "target\debug\goonzu_client.exe"
$out  = Join-Path $tmp "drv_live_canonical.log"

if (-not $env:GOONZU_USER_ID -or -not $env:GOONZU_USER_PW) {
    Write-Output "set GOONZU_USER_ID / GOONZU_USER_PW in this shell first (not stored in evidence)"
    exit 2
}
$env:GOONZU_FRONT_ADDR   = "0.tcp.ap.ngrok.io:14195"   # wire-verified FRONT
$env:GOONZU_GAME_ADDR    = "0.tcp.ap.ngrok.io:27911"   # wire-verified GAME
$env:GOONZU_CHAR_SLOT    = "0"
$env:GOONZU_MAC_ADDRESS  = "00-11-22-33-44-55"
Remove-Item $out -ErrorAction SilentlyContinue

if (-not (Test-Path $exe)) { Write-Output "build the driver first: cargo build"; exit 9 }

# The .NET Process API: Start-Process -PassThru leaves ExitCode blank on
# this box (process object never fully attached), so the exit code below
# would come back empty. cmd-redirect keeps driver output + exit code in
# ONE file, which is what makes the artifact self-contained.
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "cmd.exe"
$psi.Arguments = '/c ""' + $exe + '" > "' + $out + '" 2>&1"'
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $repo
$p = [System.Diagnostics.Process]::Start($psi)
$p.WaitForExit()
$code = $p.ExitCode

Add-Content -Path $out -Value ""
Add-Content -Path $out -Value "[evidence] driver process exit code = $code (WO-6.1 S5: 1 = runtime failure, failing stage named in the error line above)"
Add-Content -Path $out -Value "[evidence] run at $([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss')) local, binary target\debug\goonzu_client.exe from step/6.2-harness"
Write-Output "[driver] EXIT CODE = $code"
Write-Output "--- log ---"
Get-Content $out

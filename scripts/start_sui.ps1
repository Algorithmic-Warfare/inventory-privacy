# Start SUI localnet with faucet and fresh genesis
$ErrorActionPreference = "SilentlyContinue"

# Kill any existing sui processes to avoid port conflicts
$existing = Get-Process -Name "sui*" -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "[sui] Killing existing sui processes..."
    $existing | Stop-Process -Force
    Start-Sleep -Seconds 2
}

$env:RUST_LOG = "off,sui_node=info"
sui start --with-faucet --force-regenesis --epoch-duration-ms 10000

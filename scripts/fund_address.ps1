# Fund address script for Windows
$ErrorActionPreference = "Stop"

# Optional delay
$delay = if ($env:FUND_DELAY_SECONDS) { [int]$env:FUND_DELAY_SECONDS } else { 5 }
Write-Host "[fund] Delaying start for ${delay}s..."
Start-Sleep -Seconds $delay

# Ensure localnet environment exists
$envs = sui client envs 2>$null
if (-not ($envs -match "localnet")) {
    Write-Host "[fund] Creating localnet environment..."
    sui client new-env --alias localnet --rpc http://127.0.0.1:9000
}

sui client switch --env localnet
Write-Host "[fund] Switched to localnet environment."

$address = sui client active-address
Write-Host "[fund] Funding address: $address"

# Faucet retry loop
$maxAttempts = if ($env:MAX_FAUCET_ATTEMPTS) { [int]$env:MAX_FAUCET_ATTEMPTS } else { 15 }
$success = $false

for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
    Write-Host "[fund] Faucet attempt ${attempt}/${maxAttempts}..."
    try {
        sui client faucet --address $address 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "[fund] Faucet request succeeded."
            $success = $true
            break
        }
    } catch {}

    if ($attempt -eq $maxAttempts) {
        Write-Host "[fund] Faucet failed after ${maxAttempts} attempts." -ForegroundColor Red
        exit 1
    }

    $sleepTime = [Math]::Min($attempt * 2, 10)
    Write-Host "[fund] Sleeping ${sleepTime}s before retry..."
    Start-Sleep -Seconds $sleepTime
}

# Poll for balance
$maxWait = if ($env:MAX_BALANCE_WAIT) { [int]$env:MAX_BALANCE_WAIT } else { 60 }
$hasBalance = $false

for ($i = 1; $i -le $maxWait; $i++) {
    $gas = sui client gas 2>$null
    if ($gas -match "gasCoinId") {
        Write-Host "[fund] Balance confirmed at poll ${i}/${maxWait}"
        $hasBalance = $true
        break
    }
    Write-Host "[fund] Poll ${i}/${maxWait} - waiting for balance..."
    Start-Sleep -Seconds 1
}

if (-not $hasBalance) {
    Write-Host "[fund] Failed to observe funded balance after ${maxWait}s" -ForegroundColor Red
    exit 1
}

Write-Host "[fund] Address funded successfully!" -ForegroundColor Green
sui client gas

# Deploy contracts script for Windows
# Note: We don't use "Stop" because sui outputs notes to stderr which PowerShell treats as errors
$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir

Write-Host "=== Inventory Privacy Contract Deployment ===" -ForegroundColor Cyan

# Ensure localnet
sui client switch --env localnet 2>$null

# Check for verifying keys
$vksPath = Join-Path $RootDir "keys\verifying_keys.json"
if (-not (Test-Path $vksPath)) {
    Write-Host "[deploy] Verifying keys not found. Running export-vks..."
    Push-Location $RootDir
    cargo run --release --bin export-vks
    Pop-Location
}

if (-not (Test-Path $vksPath)) {
    Write-Host "[deploy] ERROR: Verifying keys still not found" -ForegroundColor Red
    exit 1
}

Write-Host "[deploy] Verifying keys ready"

# Publish Move package
Write-Host "[deploy] Publishing Move package..."
Push-Location (Join-Path $RootDir "packages\inventory")
$publishOutput = sui client publish --gas-budget 500000000 --json 2>&1 | Out-String
Pop-Location

# Extract package ID using regex
if ($publishOutput -match '"packageId"\s*:\s*"(0x[a-fA-F0-9]+)"') {
    $packageId = $matches[1]
} else {
    Write-Host "[deploy] ERROR: Failed to extract package ID" -ForegroundColor Red
    Write-Host $publishOutput
    exit 1
}

Write-Host "[deploy] Package published: $packageId"

# Load VKs
$vks = Get-Content $vksPath | ConvertFrom-Json

# Create VolumeRegistry
Write-Host "[deploy] Creating VolumeRegistry..."
$volOutput = sui client call `
    --package $packageId `
    --module volume_registry `
    --function create_and_share `
    --args "[0,5,3,8,2,10,4,15,1,6,7,12,9,20,11,25]" "0xb08a402d53183775208f9f8772791a51f6af5f7b648203b9bef158feb89b1815" `
    --gas-budget 100000000 `
    --json 2>&1 | Out-String

# Parse JSON and find VolumeRegistry object (objectType comes before objectId in JSON)
$volumeRegistryId = $null
if ($volOutput -match '"objectType"\s*:\s*"[^"]*volume_registry::VolumeRegistry"[^}]*"objectId"\s*:\s*"(0x[a-fA-F0-9]+)"') {
    $volumeRegistryId = $matches[1]
}
if ($volumeRegistryId) {
    Write-Host "[deploy] VolumeRegistry: $volumeRegistryId"
} else {
    Write-Host "[deploy] Warning: Could not extract VolumeRegistry ID"
}

# Create VerifyingKeys
Write-Host "[deploy] Creating VerifyingKeys..."
$vkOutput = sui client call `
    --package $packageId `
    --module inventory `
    --function init_verifying_keys_and_share `
    --args $vks.item_exists_vk $vks.withdraw_vk $vks.deposit_vk $vks.transfer_vk $vks.capacity_vk $vks.deposit_capacity_vk $vks.transfer_capacity_vk `
    --gas-budget 500000000 `
    --json 2>&1 | Out-String

# Find VerifyingKeys object ID (objectType comes before objectId in JSON)
$verifyingKeysId = $null
if ($vkOutput -match '"objectType"\s*:\s*"[^"]*inventory::VerifyingKeys"[^}]*"objectId"\s*:\s*"(0x[a-fA-F0-9]+)"') {
    $verifyingKeysId = $matches[1]
}
if ($verifyingKeysId) {
    Write-Host "[deploy] VerifyingKeys: $verifyingKeysId"
} else {
    Write-Host "[deploy] Warning: Could not extract VerifyingKeys ID"
}

# Save deployment info
$deployment = @{
    network = "localnet"
    packageId = $packageId
    verifyingKeysId = $verifyingKeysId
    volumeRegistryId = $volumeRegistryId
    timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
}

$deployPath = Join-Path $RootDir "keys\deployment.json"
$deployment | ConvertTo-Json | Set-Content $deployPath

# Copy to web/public for runtime access
$webPublicPath = Join-Path $RootDir "web\public\deployment.json"
$deployment | ConvertTo-Json | Set-Content $webPublicPath
Write-Host "[deploy] Copied deployment.json to web/public/"

Write-Host ""
Write-Host "=== Deployment Complete ===" -ForegroundColor Green
Write-Host "Package ID: $packageId"
Write-Host "VerifyingKeys ID: $verifyingKeysId"
Write-Host "VolumeRegistry ID: $volumeRegistryId"

Write-Host ""
Write-Host "[deploy] Web app will auto-load config from deployment.json" -ForegroundColor Green

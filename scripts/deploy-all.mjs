#!/usr/bin/env node
/**
 * Unified deployment script for inventory-privacy
 *
 * This script:
 * 1. Ensures circuit keys exist (runs trusted setup if needed)
 * 2. Exports verifying keys to JSON
 * 3. Publishes the Move package
 * 4. Initializes VolumeRegistry and VerifyingKeys on-chain
 * 5. Outputs deployment info for web frontend config
 */

import { getFullnodeUrl, SuiClient } from '@mysten/sui/client';
import { Ed25519Keypair } from '@mysten/sui/keypairs/ed25519';
import { Transaction } from '@mysten/sui/transactions';
import { fromBase64 } from '@mysten/sui/utils';
import { execSync, spawnSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.join(__dirname, '..');

function log(msg) {
  console.log(`[deploy] ${msg}`);
}

function error(msg) {
  console.error(`[deploy] ERROR: ${msg}`);
}

function hexToBytes(hex) {
  const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = [];
  for (let i = 0; i < cleanHex.length; i += 2) {
    bytes.push(parseInt(cleanHex.substr(i, 2), 16));
  }
  return bytes;
}

async function getKeypair() {
  const activeAddress = execSync('sui client active-address').toString().trim();
  log(`Active address: ${activeAddress}`);

  let homeDir = process.platform === 'win32' ? process.env.USERPROFILE : process.env.HOME;
  const keystorePath = path.join(homeDir, '.sui', 'sui_config', 'sui.keystore');
  const keystore = JSON.parse(fs.readFileSync(keystorePath, 'utf-8'));

  for (const keyBase64 of keystore) {
    const keyBytes = fromBase64(keyBase64);
    if (keyBytes[0] === 0) { // ed25519
      const privateKey = keyBytes.slice(1);
      const kp = Ed25519Keypair.fromSecretKey(privateKey);
      if (kp.getPublicKey().toSuiAddress() === activeAddress) {
        return kp;
      }
    }
  }
  throw new Error('Could not find keypair for active address');
}

async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════╗');
  console.log('║         INVENTORY PRIVACY - FULL DEPLOYMENT                   ║');
  console.log('╚═══════════════════════════════════════════════════════════════╝\n');

  // Step 0: Request gas from faucet (needed after fresh genesis)
  log('Requesting gas from faucet...');
  try {
    execSync('sui client faucet', { encoding: 'utf-8', stdio: 'pipe' });
  } catch (e) {
    // Try again - sometimes faucet needs a moment
    await new Promise(r => setTimeout(r, 2000));
    try {
      execSync('sui client faucet', { encoding: 'utf-8', stdio: 'pipe' });
    } catch (e2) {
      log('Faucet request failed, continuing anyway...');
    }
  }
  log('Gas acquired');

  // Step 1: Check/generate circuit keys
  const keysDir = path.join(ROOT_DIR, 'keys');
  const vksPath = path.join(keysDir, 'verifying_keys.json');

  if (!fs.existsSync(vksPath)) {
    log('Verifying keys not found. Running export-vks...');
    const result = spawnSync('cargo', ['run', '--release', '--bin', 'export-vks'], {
      cwd: ROOT_DIR,
      stdio: 'inherit',
      shell: true,
    });
    if (result.status !== 0) {
      error('Failed to export verifying keys');
      process.exit(1);
    }
  }

  if (!fs.existsSync(vksPath)) {
    error('Verifying keys still not found after export');
    process.exit(1);
  }

  log('Verifying keys ready');
  const vks = JSON.parse(fs.readFileSync(vksPath, 'utf-8'));

  // Step 2: Publish Move package
  log('Publishing Move package...');
  const publishResult = spawnSync('sui', [
    'client', 'publish',
    '--gas-budget', '500000000',
    '--json',
    '--silence-warnings'
  ], {
    cwd: path.join(ROOT_DIR, 'packages', 'inventory'),
    encoding: 'utf-8',
    shell: true,
  });

  // The output may contain non-JSON lines before the actual JSON
  const stdout = publishResult.stdout || '';
  const jsonMatch = stdout.match(/\{[\s\S]*\}/);

  if (!jsonMatch) {
    error('Failed to publish package - no JSON output');
    console.log('stdout:', stdout);
    console.log('stderr:', publishResult.stderr);
    process.exit(1);
  }

  let publishOutput;
  try {
    publishOutput = JSON.parse(jsonMatch[0]);
  } catch (e) {
    error('Failed to parse publish output');
    console.log(stdout);
    process.exit(1);
  }

  if (publishOutput.effects?.status?.status !== 'success') {
    error('Publish transaction failed');
    console.log(JSON.stringify(publishOutput.effects?.status, null, 2));
    process.exit(1);
  }

  const packageId = publishOutput.objectChanges?.find(c => c.type === 'published')?.packageId;
  if (!packageId) {
    error('Could not find package ID in publish output');
    console.log(JSON.stringify(publishOutput, null, 2));
    process.exit(1);
  }

  log(`Package published: ${packageId}`);

  // Step 3: Initialize on-chain objects
  const client = new SuiClient({ url: getFullnodeUrl('localnet') });
  const keypair = await getKeypair();

  // Deploy VolumeRegistry
  log('Creating VolumeRegistry...');
  const volumes = [0, 5, 3, 8, 2, 10, 4, 15, 1, 6, 7, 12, 9, 20, 11, 25];
  const registryHash = '0xb08a402d53183775208f9f8772791a51f6af5f7b648203b9bef158feb89b1815';

  const volTx = new Transaction();
  volTx.moveCall({
    target: `${packageId}::volume_registry::create_and_share`,
    arguments: [
      volTx.pure.vector('u64', volumes),
      volTx.pure.vector('u8', hexToBytes(registryHash)),
    ],
  });
  volTx.setGasBudget(100000000);

  const volResult = await client.signAndExecuteTransaction({
    signer: keypair,
    transaction: volTx,
    options: { showEffects: true, showObjectChanges: true },
  });

  const volumeRegistryId = volResult.objectChanges?.find(
    c => c.type === 'created' && c.objectType?.includes('VolumeRegistry')
  )?.objectId;

  log(`VolumeRegistry created: ${volumeRegistryId}`);

  // Deploy VerifyingKeys (using CLI - SDK has timeout issues)
  log('Creating VerifyingKeys...');
  const vkArgs = [
    vks.item_exists_vk,
    vks.withdraw_vk,
    vks.deposit_vk,
    vks.transfer_vk,
    vks.capacity_vk,
    vks.deposit_capacity_vk,
    vks.transfer_capacity_vk,
  ].join(' ');

  const vkCmd = `sui client call --package ${packageId} --module inventory --function init_verifying_keys_and_share --args ${vkArgs} --gas-budget 500000000 --json`;

  let vkOutput;
  try {
    const vkStdout = execSync(vkCmd, {
      encoding: 'utf-8',
      maxBuffer: 10 * 1024 * 1024,
      timeout: 120000,
    });
    vkOutput = JSON.parse(vkStdout.match(/\{[\s\S]*\}/)?.[0] || '{}');
  } catch (e) {
    error('VerifyingKeys transaction failed');
    console.error('Error:', e.message);
    if (e.stdout) console.error('stdout:', e.stdout.slice(0, 1000));
    if (e.stderr) console.error('stderr:', e.stderr.slice(0, 1000));
    throw e;
  }

  const verifyingKeysId = vkOutput.objectChanges?.find(
    c => c.type === 'created' && c.objectType?.includes('VerifyingKeys')
  )?.objectId;

  log(`VerifyingKeys created: ${verifyingKeysId}`);

  // Save deployment info
  const deployment = {
    network: 'localnet',
    packageId,
    verifyingKeysId,
    volumeRegistryId,
    timestamp: new Date().toISOString(),
  };

  const deploymentPath = path.join(keysDir, 'deployment.json');
  fs.writeFileSync(deploymentPath, JSON.stringify(deployment, null, 2));

  // Print summary
  console.log('\n╔═══════════════════════════════════════════════════════════════╗');
  console.log('║                    DEPLOYMENT COMPLETE                        ║');
  console.log('╚═══════════════════════════════════════════════════════════════╝\n');
  console.log('Add these to web/src/sui/config.ts localnet section:\n');
  console.log(`  packageId: '${packageId}',`);
  console.log(`  verifyingKeysId: '${verifyingKeysId}',`);
  console.log(`  volumeRegistryId: '${volumeRegistryId}',`);
  console.log(`\nOr configure via the web UI: http://localhost:5173/on-chain\n`);
  console.log(`Deployment saved to: ${deploymentPath}\n`);
}

main().catch(err => {
  error(err.message);
  process.exit(1);
});

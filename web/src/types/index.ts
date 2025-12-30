// SMT-based inventory types

export interface InventoryItem {
  item_id: number;
  quantity: number;
}

// Deprecated alias for backward compatibility
export type InventorySlot = InventoryItem;

export interface InventoryState {
  items: InventoryItem[];
  currentVolume: number;
  blinding: string;
  commitment: string | null;
}

// Legacy type alias
export interface Inventory {
  slots: InventoryItem[];
  blinding: string;
  commitment: string | null;
}

export interface ProofResult {
  proof: string;
  public_inputs: string[];
}

export interface StateTransitionResult extends ProofResult {
  new_commitment: string;
  new_volume: number;
}

// Legacy aliases for backward compatibility
export interface WithdrawResult extends ProofResult {
  new_commitment: string;
}

export interface DepositResult extends ProofResult {
  new_commitment: string;
}

export interface TransferResult extends ProofResult {
  src_new_commitment: string;
  dst_new_commitment: string;
}

export interface ApiError {
  error: string;
}

// SMT can handle up to 4096 items with depth 12, but we limit UI display
export const MAX_DISPLAY_ITEMS = 100;

export const ITEM_NAMES: Record<number, string> = {
  1: 'Gold Ore',
  2: 'Iron Ingot',
  3: 'Diamond',
  4: 'Wood',
  5: 'Stone',
  6: 'Coal',
  7: 'Copper',
  8: 'Silver',
  9: 'Emerald',
  10: 'Ruby',
  11: 'Sapphire',
  12: 'Steel',
  13: 'Titanium',
  14: 'Platinum',
  15: 'Crystal',
  16: 'Obsidian',
};

export const ITEM_COLORS: Record<number, string> = {
  1: 'bg-yellow-400',
  2: 'bg-gray-400',
  3: 'bg-cyan-300',
  4: 'bg-amber-600',
  5: 'bg-stone-400',
  6: 'bg-gray-800',
  7: 'bg-orange-400',
  8: 'bg-slate-300',
  9: 'bg-emerald-400',
  10: 'bg-red-400',
  11: 'bg-blue-400',
  12: 'bg-zinc-500',
  13: 'bg-slate-400',
  14: 'bg-amber-200',
  15: 'bg-purple-300',
  16: 'bg-violet-900',
};

// Volume per unit for each item type (index 0 = empty slot = 0 volume)
export const ITEM_VOLUMES: Record<number, number> = {
  0: 0,   // empty
  1: 5,   // Gold Ore
  2: 3,   // Iron Ingot
  3: 8,   // Diamond
  4: 2,   // Wood
  5: 10,  // Stone
  6: 4,   // Coal
  7: 15,  // Copper
  8: 1,   // Silver
  9: 6,   // Emerald
  10: 7,  // Ruby
  11: 12, // Sapphire
  12: 9,  // Steel
  13: 20, // Titanium
  14: 11, // Platinum
  15: 25, // Crystal
  16: 30, // Obsidian
};

export interface CapacityInfo {
  usedVolume: number;
  maxCapacity: number;
  availableSpace: number;
}

// Calculate used volume from inventory items
export function calculateUsedVolume(items: InventoryItem[]): number {
  return items.reduce((total, item) => {
    const volumePerUnit = ITEM_VOLUMES[item.item_id] ?? 0;
    return total + (item.quantity * volumePerUnit);
  }, 0);
}

// Check if deposit would exceed capacity
export function canDeposit(
  currentItems: InventoryItem[],
  itemId: number,
  amount: number,
  maxCapacity: number
): boolean {
  if (maxCapacity === 0) return true; // No capacity limit
  const currentVolume = calculateUsedVolume(currentItems);
  const additionalVolume = (ITEM_VOLUMES[itemId] ?? 0) * amount;
  return (currentVolume + additionalVolume) <= maxCapacity;
}

// Get item volume
export function getItemVolume(itemId: number): number {
  return ITEM_VOLUMES[itemId] ?? 0;
}

// Convert items to API format
export function itemsToApiFormat(items: InventoryItem[]): { item_id: number; quantity: number }[] {
  return items.filter(item => item.quantity > 0);
}

// Get volume registry as array for on-chain operations
// Returns volumes for item IDs 0-15 in order
export function getVolumeRegistryArray(): number[] {
  return [
    ITEM_VOLUMES[0] ?? 0,   // 0 - empty
    ITEM_VOLUMES[1] ?? 0,   // 5 - Gold Ore
    ITEM_VOLUMES[2] ?? 0,   // 3 - Iron Ingot
    ITEM_VOLUMES[3] ?? 0,   // 8 - Diamond
    ITEM_VOLUMES[4] ?? 0,   // 2 - Wood
    ITEM_VOLUMES[5] ?? 0,   // 10 - Stone
    ITEM_VOLUMES[6] ?? 0,   // 4 - Coal
    ITEM_VOLUMES[7] ?? 0,   // 15 - Copper
    ITEM_VOLUMES[8] ?? 0,   // 1 - Silver
    ITEM_VOLUMES[9] ?? 0,   // 6 - Emerald
    ITEM_VOLUMES[10] ?? 0,  // 7 - Ruby
    ITEM_VOLUMES[11] ?? 0,  // 12 - Sapphire
    ITEM_VOLUMES[12] ?? 0,  // 9 - Steel
    ITEM_VOLUMES[13] ?? 0,  // 20 - Titanium
    ITEM_VOLUMES[14] ?? 0,  // 11 - Platinum
    ITEM_VOLUMES[15] ?? 0,  // 25 - Crystal
  ];
}

// Get registry root hash (dummy for demo - in production from on-chain)
export function getRegistryRoot(): string {
  return '0x0000000000000000000000000000000000000000000000000000000000000000';
}

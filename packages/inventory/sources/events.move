/// Event definitions for inventory operations.
/// Events are emitted by the main inventory module but defined here for clarity.
module inventory::events {
    // Note: Most events are defined inline in inventory.move and registry.move
    // This module provides additional event types for extended functionality.

    /// Emitted when inventory ownership is transferred
    public struct OwnershipTransferred has copy, drop {
        inventory_id: ID,
        old_owner: address,
        new_owner: address,
    }

    /// Emitted when batch operations are performed
    public struct BatchOperation has copy, drop {
        inventory_id: ID,
        operation_type: u8, // 0 = withdraw, 1 = deposit
        num_items: u64,
    }

    /// Emitted for audit/compliance purposes
    public struct AuditLog has copy, drop {
        inventory_id: ID,
        operation: vector<u8>,
        timestamp: u64,
        actor: address,
    }
}

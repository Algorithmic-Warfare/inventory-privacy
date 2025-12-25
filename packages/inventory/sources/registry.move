/// Registry for tracking and creating private inventories.
module inventory::registry {
    use sui::event;
    use inventory::inventory::{Self, PrivateInventory, VerifyingKeys};

    // ============ Error Codes ============

    const ENotAdmin: u64 = 0;

    // ============ Structs ============

    /// Registry for tracking all inventories.
    /// Optional - provides discoverability and stats.
    public struct InventoryRegistry has key {
        id: UID,
        /// Total number of inventories created
        count: u64,
        /// Admin address
        admin: address,
    }

    /// Admin capability for registry management
    public struct AdminCap has key, store {
        id: UID,
    }

    // ============ Events ============

    /// Emitted when registry is initialized
    public struct RegistryInitialized has copy, drop {
        registry_id: ID,
        admin: address,
    }

    /// Emitted when inventory is spawned via registry
    public struct InventorySpawned has copy, drop {
        registry_id: ID,
        inventory_id: ID,
        owner: address,
        inventory_number: u64,
    }

    // ============ Initialization ============

    /// Initialize the registry (called once during deployment)
    fun init(ctx: &mut TxContext) {
        let admin = tx_context::sender(ctx);

        let registry = InventoryRegistry {
            id: object::new(ctx),
            count: 0,
            admin,
        };

        let admin_cap = AdminCap {
            id: object::new(ctx),
        };

        event::emit(RegistryInitialized {
            registry_id: object::id(&registry),
            admin,
        });

        transfer::share_object(registry);
        transfer::transfer(admin_cap, admin);
    }

    // ============ Registry Functions ============

    /// Spawn a new private inventory through the registry
    public fun spawn_inventory(
        registry: &mut InventoryRegistry,
        initial_commitment: vector<u8>,
        ctx: &mut TxContext,
    ): PrivateInventory {
        registry.count = registry.count + 1;

        let inv = inventory::create(initial_commitment, ctx);

        event::emit(InventorySpawned {
            registry_id: object::id(registry),
            inventory_id: object::id(&inv),
            owner: tx_context::sender(ctx),
            inventory_number: registry.count,
        });

        inv
    }

    /// Get the total count of inventories
    public fun count(registry: &InventoryRegistry): u64 {
        registry.count
    }

    /// Get the admin address
    public fun admin(registry: &InventoryRegistry): address {
        registry.admin
    }

    // ============ Admin Functions ============

    /// Transfer admin rights
    public fun transfer_admin(
        registry: &mut InventoryRegistry,
        _admin_cap: &AdminCap,
        new_admin: address,
        ctx: &TxContext,
    ) {
        assert!(registry.admin == tx_context::sender(ctx), ENotAdmin);
        registry.admin = new_admin;
    }
}

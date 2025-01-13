use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

const MAX_ITEMS: usize = 64;

#[derive(Pod, Zeroable, Copy, Clone)]
#[repr(C)]
pub struct Registry {
    pub registered_segmenters: [Pubkey; MAX_ITEMS],
}

impl Registry {
    pub const PROGRAM_ID: Pubkey = pubkey!("SRegZsVZDDqwc7W5iMUSsmKNnXzgfczKzFpimRp5iWw");
    pub const DISCRIMINATOR: [u8; 8] = [47, 174, 110, 246, 184, 182, 252, 218];

    pub fn is_segmenter_registered(&self, key: &Pubkey) -> bool {
        self.registered_segmenters.binary_search(key).is_ok()
    }

    pub fn deserialize(bytes: &[u8]) -> &Self {
        bytemuck::from_bytes(&bytes[8..])
    }
}

pub fn is_invoked_by_segmenter(registry: &AccountInfo<'_>, segmenter: &AccountInfo<'_>) -> bool {
    if *registry.owner != Registry::PROGRAM_ID {
        return false;
    }
    if !segmenter.is_signer {
        return false;
    }

    let registry_account_data = registry.data.borrow();
    if registry_account_data[..8] != Registry::DISCRIMINATOR {
        return false;
    }

    let registry_state = Registry::deserialize(&registry_account_data);
    registry_state.is_segmenter_registered(segmenter.key)
}

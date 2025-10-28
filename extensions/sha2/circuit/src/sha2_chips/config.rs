use openvm_sha2_air::{Sha256Config, Sha2BlockHasherSubairConfig, Sha384Config, Sha512Config};
use sha2::{
    compress256, compress512, digest::generic_array::GenericArray, Digest, Sha256, Sha384, Sha512,
};

use crate::{Sha2BlockHasherVmConfig, Sha2MainChipConfig};

pub const SHA2_REGISTER_READS: usize = 3;
pub const SHA2_READ_SIZE: usize = 4;
pub const SHA2_WRITE_SIZE: usize = 4;

pub trait Sha2Config: Sha2MainChipConfig + Sha2BlockHasherVmConfig {
    // --- Required ---
    /// Number of bits used to store the message length (part of the message padding)
    const MESSAGE_LENGTH_BITS: usize;

    fn compress(state: &mut [u8], input: &[u8]);
    fn hash(message: &[u8]) -> Vec<u8>;
}

impl Sha2Config for Sha256Config {
    const MESSAGE_LENGTH_BITS: usize = 64;

    fn compress(state: &mut [u8], input: &[u8]) {
        let state: &mut [u32; 8] = unsafe { &mut *(state.as_mut_ptr() as *mut [u32; 8]) };
        let input_array = GenericArray::from_slice(input);
        compress256(state, &[*input_array]);
    }

    fn hash(message: &[u8]) -> Vec<u8> {
        Sha256::digest(message).to_vec()
    }
}

impl Sha2Config for Sha512Config {
    const MESSAGE_LENGTH_BITS: usize = 128;

    fn compress(state: &mut [u8], input: &[u8]) {
        let state: &mut [u64; 8] = unsafe { &mut *(state.as_mut_ptr() as *mut [u64; 8]) };
        let input_array = GenericArray::from_slice(input);
        compress512(state, &[*input_array]);
    }

    fn hash(message: &[u8]) -> Vec<u8> {
        Sha512::digest(message).to_vec()
    }
}

impl Sha2Config for Sha384Config {
    const MESSAGE_LENGTH_BITS: usize = Sha512Config::MESSAGE_LENGTH_BITS;

    fn compress(state: &mut [u8], input: &[u8]) {
        let state: &mut [u64; 8] = unsafe { &mut *(state.as_mut_ptr() as *mut [u64; 8]) };
        let input_array = GenericArray::from_slice(input);
        compress512(state, &[*input_array]);
    }

    fn hash(message: &[u8]) -> Vec<u8> {
        Sha384::digest(message).to_vec()
    }
}

use core::cmp::min;

const SHA256_STATE_BYTES: usize = 32;
const SHA256_BLOCK_BYTES: usize = 64;
const SHA256_DIGEST_BYTES: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct Sha256 {
    // the current hasher state
    state: [u8; SHA256_STATE_BYTES],
    // the next block of input
    buffer: [u8; SHA256_BLOCK_BYTES],
    // idx of next byte to write to buffer
    idx: usize,
    // accumulated length of the input data
    len: usize,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [0; SHA256_STATE_BYTES],
            buffer: [0; SHA256_BLOCK_BYTES],
            idx: 0,
            len: 0,
        }
    }

    pub fn update(&mut self, mut input: &[u8]) {
        self.len += input.len();
        while !input.is_empty() {
            let to_copy = min(input.len(), SHA256_BLOCK_BYTES - self.idx);
            self.buffer[self.idx..self.idx + to_copy].copy_from_slice(&input[..to_copy]);
            self.idx += to_copy;
            if self.idx == SHA256_BLOCK_BYTES {
                self.idx = 0;
                self.compress();
            }
            input = &input[to_copy..];
        }
    }

    pub fn finalize(&mut self) -> [u8; SHA256_DIGEST_BYTES] {
        self.update(&[0x80]);
        while self.idx < SHA256_BLOCK_BYTES - 8 {
            self.buffer[self.idx] = 0;
            self.idx += 1;
        }
        self.buffer[SHA256_BLOCK_BYTES - 8..SHA256_BLOCK_BYTES]
            .copy_from_slice(&(self.len as u64).to_be_bytes());
        self.compress();
        self.state
    }

    fn compress(&mut self) {
        openvm_sha2_guest::zkvm_sha256_impl(
            self.state.as_ptr(),
            self.buffer.as_ptr(),
            self.state.as_mut_ptr() as *mut u8,
        );
    }
}

const SHA512_STATE_BYTES: usize = 64;
const SHA512_BLOCK_BYTES: usize = 128;
const SHA512_DIGEST_BYTES: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct Sha512 {
    // the current hasher state
    state: [u8; SHA512_STATE_BYTES],
    // the next block of input
    buffer: [u8; SHA512_BLOCK_BYTES],
    // idx of next byte to write to buffer
    idx: usize,
    // accumulated length of the input data
    len: usize,
}

impl Default for Sha512 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha512 {
    pub fn new() -> Self {
        Self {
            state: [0; SHA512_STATE_BYTES],
            buffer: [0; SHA512_BLOCK_BYTES],
            idx: 0,
            len: 0,
        }
    }

    pub fn update(&mut self, mut input: &[u8]) {
        self.len += input.len();
        while !input.is_empty() {
            let to_copy = min(input.len(), SHA512_BLOCK_BYTES - self.idx);
            self.buffer[self.idx..self.idx + to_copy].copy_from_slice(&input[..to_copy]);
            self.idx += to_copy;
            if self.idx == SHA512_BLOCK_BYTES {
                self.idx = 0;
                self.compress();
            }
            input = &input[to_copy..];
        }
    }

    pub fn finalize(&mut self) -> [u8; SHA512_DIGEST_BYTES] {
        self.update(&[0x80]);
        while self.idx < SHA512_BLOCK_BYTES - 8 {
            self.buffer[self.idx] = 0;
            self.idx += 1;
        }
        self.buffer[SHA512_BLOCK_BYTES - 16..SHA512_BLOCK_BYTES]
            .copy_from_slice(&(self.len as u128).to_be_bytes());
        self.compress();
        self.state
    }

    fn compress(&mut self) {
        openvm_sha2_guest::zkvm_sha512_impl(
            self.state.as_ptr(),
            self.buffer.as_ptr(),
            self.state.as_mut_ptr() as *mut u8,
        );
    }
}

const SHA384_STATE_BYTES: usize = 64;
const SHA384_BLOCK_BYTES: usize = 128;
const SHA384_DIGEST_BYTES: usize = 48;

#[derive(Debug, Clone, Copy)]
pub struct Sha384 {
    inner: Sha512,
}

impl Default for Sha384 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha384 {
    pub fn new() -> Self {
        Self {
            inner: Sha512::new(),
        }
    }

    pub fn update(&mut self, input: &[u8]) {
        self.inner.update(input);
    }

    pub fn finalize(&mut self) -> [u8; SHA384_DIGEST_BYTES] {
        let digest = self.inner.finalize();
        digest[..SHA384_DIGEST_BYTES].try_into().unwrap()
    }

    fn compress(&mut self) {
        self.inner.compress();
    }
}

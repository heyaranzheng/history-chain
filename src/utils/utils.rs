
#[cfg(test)]
pub mod block_tools {
    use crate::block::{Block, Carrier, Digester};
    use crate::hash::HashValue;
    use crate::errors::HError;
    
    use rand::Rng;
    use chrono::Utc;
    use sha2::Sha256;

    ///returns a random hash value
    pub fn random_hash() -> Hashvalue {
        let mut rng = rand::thread_rng();
        let random_hash: [u8; 32] = rng.r#gen();
        let timestamp = Utc::now().timestamp().to_be_bytes();

        let mut hasher = Sha256::new();
        hasher.update(&random_hash);
        hasher.update(&timestamp);

        let result: HashValue = hasher.finalize().into();
        result
    }

    ///return a random block with a random hash value
    




    


}
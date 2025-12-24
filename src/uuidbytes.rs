use uuid::Uuid;


pub type UuidBytes = [u8; 16];

///create a new Object for initialization.
pub trait  Init{
    fn new() -> Self;
}

impl Init for UuidBytes {
    fn new() -> Self {
        let uuid = Uuid::now_v7();
        let uuid_bytes = uuid.as_bytes().clone().into();
        uuid_bytes
    }
    
}





#[cfg(test)]
mod tests {
    use super:: *;
    use std::thread::sleep;

    #[test]
    fn test_default_uuid() {
        let uuid_bytes1 = UuidBytes::new();
        sleep(std::time::Duration::from_millis(10));
        let uuid_bytes2 = UuidBytes::new();

        assert!(uuid_bytes1 != uuid_bytes2);
    }        

}

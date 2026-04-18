use uuid::Uuid;

use crate::domain::port::id_generator::IdGenerator;

pub struct Uuid4IdGenerator;

impl IdGenerator for Uuid4IdGenerator {
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

//! System-boundary adapters — clock and id generator.

pub mod clock;
pub mod id_generator;

pub use clock::SystemClock;
pub use id_generator::Uuid4IdGenerator;

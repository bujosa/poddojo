//! Everything PodDojo does to the outside world: pick the machine that runs Docker (any Docker
//! context: this Mac, a home server over SSH…), run lesson commands there with a fixed lab
//! environment, manage the isolated lab cluster, and read containers, images and Kubernetes objects
//! for the explorers.
//!
//! Safety: commands always get `DOCKER_CONTEXT` set to the chosen machine and `KUBECONFIG` set to
//! PodDojo's own kubeconfig file, so nothing here can reach the user's other clusters.

pub mod cluster;
pub mod doctor;
pub mod exec;
pub mod explore;
pub mod lab;
pub mod paths;
pub mod profiles;
pub mod settings;
pub mod targets;
pub mod terminal;

pub use exec::{LabEnv, run};
pub use paths::Paths;
pub use profiles::{Profile, Profiles};
pub use settings::Settings;
pub use targets::Target;

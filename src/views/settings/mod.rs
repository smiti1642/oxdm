pub(crate) mod identification;
mod maintenance;
mod network;
pub(crate) mod time;
mod users;

pub use identification::IdentificationTab;
pub use maintenance::MaintenanceTab;
pub use network::NetworkTab;
pub use time::TimeTab;
pub use users::UsersTab;

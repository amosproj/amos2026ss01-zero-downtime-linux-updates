mod apps;
mod logs;
mod os;
mod ping;
mod register;

#[derive(Clone)]
pub struct DataStore {
    connection: sea_orm::DatabaseConnection,
}

impl DataStore {
    pub fn new(connection: sea_orm::DatabaseConnection) -> Self {
        Self { connection }
    }
}

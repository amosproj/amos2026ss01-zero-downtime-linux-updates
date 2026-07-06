mod apps;
mod ping;

#[derive(Clone)]
pub struct DataStore {
    connection: sea_orm::DatabaseConnection,
}

impl DataStore {
    pub fn new(connection: sea_orm::DatabaseConnection) -> Self {
        Self { connection }
    }
}

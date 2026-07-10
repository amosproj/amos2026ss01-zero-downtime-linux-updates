/// POST /register - Try and register a device
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PostBody {
    pub uuid: String,
    pub serial_number: String,
    pub endorsement_public_key: String,
    pub signing_public_key: String,
}

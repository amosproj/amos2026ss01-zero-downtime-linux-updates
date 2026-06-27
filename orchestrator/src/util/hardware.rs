use std::fs;

use tracing::debug;
use uuid::Uuid;

const DMI_UUID_PATH: &str = "/sys/class/dmi/id/product_uuid";
const DMI_SERIAL_PATH: &str = "/sys/class/dmi/id/product_serial";

/// Reads the device UUID from DMI/SMBIOS.
/// Returns Err if the file is missing, empty, contains a placeholder, or is not a valid UUID.
pub fn read_device_uuid() -> anyhow::Result<String> {
    let value = read_dmi_field(DMI_UUID_PATH, "device UUID")?;
    Uuid::parse_str(&value).map_err(|e| {
        anyhow::anyhow!(
            "DMI device UUID is not a valid UUID format: '{}': {}",
            value,
            e
        )
    })?;
    Ok(value)
}

/// Reads the serial number from DMI/SMBIOS.
/// Returns Err if the file is missing, empty, or contains a placeholder.
pub fn read_serial_number() -> anyhow::Result<String> {
    read_dmi_field(DMI_SERIAL_PATH, "serial number")
}

fn read_dmi_field(path: &str, field_name: &str) -> anyhow::Result<String> {
    let value = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Could not read {} from {}: {}", field_name, path, e))?;
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "Not Specified" || trimmed == "To Be Filled By O.E.M." {
        anyhow::bail!(
            "DMI {} at {} has no valid value: '{}'",
            field_name,
            path,
            trimmed
        );
    }
    debug!("Read DMI {} from {}: {}", field_name, path, trimmed);
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_dmi_field_missing_file() {
        let result = read_dmi_field("/nonexistent/path", "test");
        assert!(result.is_err());
    }

    #[test]
    fn read_dmi_field_placeholder_not_specified() {
        let trimmed = "Not Specified";
        assert!(
            trimmed.is_empty() || trimmed == "Not Specified" || trimmed == "To Be Filled By O.E.M."
        );
    }

    #[test]
    fn uuid_format_valid() {
        assert!(Uuid::parse_str("12345678-1234-1234-1234-123456789abc").is_ok());
    }

    #[test]
    fn uuid_format_invalid_placeholder() {
        assert!(Uuid::parse_str("not-a-uuid").is_err());
        assert!(Uuid::parse_str("12345678-1234-1234-1234").is_err());
        assert!(Uuid::parse_str("").is_err());
    }
}

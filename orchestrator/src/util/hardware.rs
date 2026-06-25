use std::fs;

use tracing::debug;

const DMI_UUID_PATH: &str = "/sys/class/dmi/id/product_uuid";
const DMI_SERIAL_PATH: &str = "/sys/class/dmi/id/product_serial";

/// Reads the device UUID from DMI/SMBIOS.
/// Returns Err if the file is missing, empty, or contains a placeholder.
pub fn read_device_uuid() -> anyhow::Result<String> {
    read_dmi_field(DMI_UUID_PATH, "device UUID")
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
}

use crate::json_value::{as_i64, as_str};
use crate::models::{BoundDevice, DiscoveredAtaDevice};
use crate::remote_preset::remote_presets_from_node;
use serde_json::Value;

pub(crate) fn flatten_ata_devices(values: Vec<Value>) -> Vec<DiscoveredAtaDevice> {
    let mut devices = Vec::new();
    for entry in values {
        if let Some(structure) = entry.get("Structure") {
            let building_id = structure
                .get("BuildingID")
                .and_then(as_i64)
                .or_else(|| entry.get("BuildingID").and_then(as_i64))
                .unwrap_or(0);
            collect_ata_devices(structure, building_id, &mut devices);
        }
    }
    devices
}

fn collect_ata_devices(
    container: &Value,
    default_building_id: i64,
    out: &mut Vec<DiscoveredAtaDevice>,
) {
    if let Some(devices) = container.get("Devices").and_then(Value::as_array) {
        for node in devices {
            if let Some(device) = discovered_ata_device_from_node(node, default_building_id) {
                out.push(device);
            }
        }
    }

    for key in ["Areas", "Floors"] {
        if let Some(children) = container.get(key).and_then(Value::as_array) {
            for child in children {
                let building_id = child
                    .get("BuildingID")
                    .and_then(as_i64)
                    .unwrap_or(default_building_id);
                collect_ata_devices(child, building_id, out);
            }
        }
    }
}

fn discovered_ata_device_from_node(
    node: &Value,
    default_building_id: i64,
) -> Option<DiscoveredAtaDevice> {
    let dev = node.get("Device")?.as_object()?;
    let device_id = dev.get("DeviceID").and_then(as_i64)?;
    let device_type = dev.get("DeviceType").and_then(as_i64).unwrap_or(-1);
    if device_type != 0 {
        return None;
    }

    let name = dev
        .get("DeviceName")
        .and_then(as_str)
        .or_else(|| node.get("DeviceName").and_then(as_str))
        .unwrap_or("Unnamed device");
    let building_id = dev
        .get("BuildingID")
        .and_then(as_i64)
        .or_else(|| node.get("BuildingID").and_then(as_i64))
        .unwrap_or(default_building_id);

    Some(DiscoveredAtaDevice {
        device: BoundDevice {
            name: name.to_string(),
            building_id,
            device_id,
        },
        presets: remote_presets_from_node(node),
        raw: node.clone(),
    })
}

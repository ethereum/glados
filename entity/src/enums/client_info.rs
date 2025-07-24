use sea_orm::{entity::prelude::*, ColIdx, QueryResult, TryGetError, TryGetable};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use strum::{Display, EnumProperty};

// Client

#[derive(Debug, Clone, Display, Deserialize, EnumIter, EnumProperty)]
#[strum(serialize_all = "lowercase")]
pub enum Client {
    #[strum(props(color = "#9B59B6", name = "Trin", placeholder = "false"))]
    Trin,
    #[strum(props(color = "#3498DB", name = "Nimbus", placeholder = "false"))]
    Nimbus,
    #[strum(props(color = "#2E8C47", name = "Samba", placeholder = "false"))]
    Samba,
    #[strum(props(color = "#DA251D", name = "Shisui", placeholder = "false"))]
    Shisui,
    #[strum(props(color = "#E67E22", name = "Ultralight", placeholder = "false"))]
    Ultralight,
    #[strum(props(color = "#808080", name = "Other", placeholder = "true"))]
    Other,
    #[strum(props(color = "#BBBBBB", name = "Unknown", placeholder = "true"))]
    Unknown,
}

impl From<String> for Client {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "nimbus" => Client::Nimbus,
            "samba" => Client::Samba,
            "shisui" => Client::Shisui,
            "trin" => Client::Trin,
            "ultralight" => Client::Ultralight,
            _ => Client::Other,
        }
    }
}

impl From<Option<String>> for Client {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Client::Unknown,
        }
    }
}

impl TryGetable for Client {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value: Option<String> = res.try_get_by(index)?;
        Ok(value.into())
    }
}

impl Serialize for Client {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("Client", 3)?;
        s.serialize_field("slug", &self.to_string())?;
        s.serialize_field("name", &self.get_str("name"))?;
        s.serialize_field("color", &self.get_str("color"))?;
        s.end()
    }
}

// CpuArchitecture

#[derive(Debug, Clone, Display, Deserialize, EnumIter)]
pub enum CpuArchitecture {
    X86_64,
    AArch64,
    Unknown,
    Other,
}
impl From<String> for CpuArchitecture {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "amd64" | "x64" | "x86_64" => CpuArchitecture::X86_64,
            "aarch64" | "ARM64" => CpuArchitecture::AArch64,
            _ => CpuArchitecture::Other,
        }
    }
}
impl From<Option<String>> for CpuArchitecture {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(value) => value.into(),
            None => CpuArchitecture::Unknown,
        }
    }
}
impl TryGetable for CpuArchitecture {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value: Option<String> = res.try_get_by(index)?;
        Ok(value.into())
    }
}

// OperatingSystem

#[derive(Debug, Clone, Display, Deserialize, EnumIter, EnumProperty)]
pub enum OperatingSystem {
    #[strum(props(color = "#22AC66", name = "Linux"))]
    Linux,
    #[strum(props(color = "#F5A623", name = "macOS"))]
    MacOS,
    #[strum(props(color = "#0078D7", name = "Windows"))]
    Windows,
    #[strum(props(color = "#808080", name = "Other"))]
    Other,
    #[strum(props(color = "#BBBBBB", name = "Unknown"))]
    Unknown,
}
impl From<String> for OperatingSystem {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "linux" => OperatingSystem::Linux,
            "darwin" | "macos" => OperatingSystem::MacOS,
            "windows" => OperatingSystem::Windows,
            _ => OperatingSystem::Other,
        }
    }
}

impl From<Option<String>> for OperatingSystem {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(value) => value.into(),
            None => OperatingSystem::Unknown,
        }
    }
}

impl TryGetable for OperatingSystem {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value: Option<String> = res.try_get_by(index)?;
        Ok(value.into())
    }
}

impl Serialize for OperatingSystem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("OperatingSystem", 3)?;
        s.serialize_field("slug", &self.to_string())?;
        s.serialize_field("name", &self.get_str("name"))?;
        s.serialize_field("color", &self.get_str("color"))?;
        s.end()
    }
}

// Version

#[derive(Debug, Clone, Serialize)]
pub struct Version(String);

impl From<String> for Version {
    fn from(value: String) -> Self {
        // Versions are not completely sanitized, to allow for non-numeric only versions
        Version(value.strip_prefix('v').unwrap_or(&value).to_string())
    }
}

impl From<Option<String>> for Version {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(value) => value.into(),
            None => "Unknown".to_string().into(),
        }
    }
}

impl TryGetable for Version {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value: Option<String> = res.try_get_by(index)?;
        Ok(value.into())
    }
}

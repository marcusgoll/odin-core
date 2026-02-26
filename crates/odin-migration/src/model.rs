use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestSectionRef {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UserDataManifest {
    pub schema_version: u32,
    pub user_data_model_version: u32,
    pub skills: Option<ManifestSectionRef>,
    pub learnings: Option<ManifestSectionRef>,
    pub runtime: Option<ManifestSectionRef>,
    pub checkpoints: Option<ManifestSectionRef>,
    pub events: Option<ManifestSectionRef>,
    pub opaque: Option<ManifestSectionRef>,
    pub quarantine: Option<ManifestSectionRef>,
    pub meta: Option<ManifestSectionRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillPackMetadata {
    pub schema_version: u32,
    pub pack_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LearningPackMetadata {
    pub schema_version: u32,
    pub pack_id: String,
}

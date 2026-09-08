use serde::Deserialize;
#[derive(Deserialize, Clone, Debug)]
pub struct ToolPurposes0Config {
    #[serde(alias = "purposes")]
    pub purposes: Vec<String>,
    #[serde(alias = "tool")]
    pub tool: String,
}
#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(alias = "allowedPurposes")]
    pub allowed_purposes: Option<Vec<String>>,
    #[serde(alias = "claimName")]
    pub claim_name: Option<String>,
    #[serde(alias = "enforceMethods")]
    pub enforce_methods: Option<Vec<String>>,
    #[serde(alias = "failMode")]
    pub fail_mode: Option<String>,
    #[serde(alias = "headerName")]
    pub header_name: Option<String>,
    #[serde(alias = "source")]
    pub source: Option<String>,
    #[serde(alias = "toolPurposes")]
    pub tool_purposes: Option<Vec<ToolPurposes0Config>>,
}
#[pdk::hl::entrypoint_flex]
fn init(abi: &dyn pdk::flex_abi::api::FlexAbi) -> Result<(), anyhow::Error> {
    abi.setup()?;
    Ok(())
}

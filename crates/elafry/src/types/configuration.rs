use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Configuration {
    pub tasks: Vec<Task>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Task {
    pub id: uuid::Uuid,
    pub actions: Action,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Action {
    #[serde(rename = "blocking")]
    Blocking(Vec<BlockingAction>),
    #[serde(rename = "non-blocking")]
    NonBlocking(Vec<NonBlockingAction>),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BlockingAction {
    pub id: uuid::Uuid,
    pub data: BlockingData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct NonBlockingAction {
    pub id: uuid::Uuid,
    pub data: NonBlockingData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum BlockingData {
    #[serde(rename = "start-component")]
    StartComponent(StartComponentData),
    #[serde(rename = "stop-component")]
    StopComponent(StopComponentData),
    #[serde(rename = "add-route")]
    AddRoute(AddRouteData),
    #[serde(rename = "remove-route")]
    RemoveRoute(RemoveRouteData),
    #[serde(rename = "set-schedule")]
    SetSchedule(SetScheduleData),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum NonBlockingData {
    #[serde(rename = "add-component")]
    AddComponent(AddComponentData),
    #[serde(rename = "remove-component")]
    RemoveComponent(RemoveComponentData),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AddComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
    pub component: String,
    pub core: usize,
    pub version: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StartComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StopComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RemoveComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AddRouteData {
    pub source: RouteEndpoint,
    pub target: RouteEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub struct RemoveRouteData {
    pub source: RouteEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub struct RouteEndpoint {
    pub endpoint: Endpoint,
    #[serde(rename = "channel-id")]
    pub channel_id: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub enum Endpoint {
    #[serde(rename = "component-id")]
    Component(uuid::Uuid),
    #[serde(rename = "address")]
    Address(String),
    #[serde(rename = "runner")]
    Runner,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SetScheduleData {
    pub frequency: u64,
    #[serde(rename = "major-frames")]
    pub major_frames: Vec<MajorFrame>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MajorFrame {
    #[serde(rename = "minor-frames")]
    pub minor_frames: Vec<MinorFrame>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MinorFrame {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
    pub deadline: u64,
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Configuration {
    pub tasks: Vec<Task>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Task {
    pub id: uuid::Uuid,
    pub actions: Vec<Action>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Action {
    AddComponent(AddComponentAction),
    StartComponent(StartComponentAction),
    StopComponent(StopComponentAction),
    RemoveComponent(RemoveComponentAction),
    AddRoute(AddRouteAction),
    RemoveRoute(RemoveRouteAction),
    SetSchedule(SetScheduleAction),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AddComponentAction {
    pub id: uuid::Uuid,
    pub data: AddComponentData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AddComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
    pub component: String,
    pub core: usize,
    pub version: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct StartComponentAction {
    pub id: uuid::Uuid,
    pub data: StartComponentData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct StartComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct StopComponentAction {
    pub id: uuid::Uuid,
    pub data: StopComponentData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct StopComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RemoveComponentAction {
    pub id: uuid::Uuid,
    pub data: RemoveComponentData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RemoveComponentData {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AddRouteAction {
    pub id: uuid::Uuid,
    pub data: AddRouteData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AddRouteData {
    pub source: RouteEndpoint,
    pub target: RouteEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RemoveRouteAction {
    pub id: uuid::Uuid,
    pub data: RemoveRouteData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub struct RemoveRouteData {
    pub source: RouteEndpoint,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Eq, Hash, Clone)]
pub struct RouteEndpoint {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
    #[serde(rename = "channel-id")]
    pub channel_id: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SetScheduleAction {
    pub id: uuid::Uuid,
    pub data: SetScheduleData,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SetScheduleData {
    pub frequency: u64,
    #[serde(rename = "major-frames")]
    pub major_frames: Vec<MajorFrame>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct MajorFrame {
    #[serde(rename = "minor-frames")]
    pub minor_frames: Vec<MinorFrame>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct MinorFrame {
    #[serde(rename = "component-id")]
    pub component_id: uuid::Uuid,
}

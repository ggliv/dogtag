use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct Subject {
    pub title: Option<String>,
    pub courses: HashMap<String, Course>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Course {
    pub title: String,
    pub description: String,
    pub credits: (f32, f32),
    pub sections: Vec<Section>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Section {
    pub crn: String,
    pub instructors: HashSet<String>,
    pub schedule: Vec<ScheduleItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScheduleItem {
    pub time: Option<(String, String)>,
    pub days: Vec<char>,
    pub location: Option<String>,
}

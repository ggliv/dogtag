use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct Subject {
    pub title: String,
    pub courses: HashMap<String, Course>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Course {
    pub title: String,
    pub description: String,
    pub credits: (String, String),
    pub sections: Vec<Section>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Section {
    pub crn: String,
    pub instructors: HashSet<String>,
    pub schedules: Vec<Schedule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub times: (String, String),
    pub days: String,
    pub location: String,
}

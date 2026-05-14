use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenerateRequest {
    pub seed: u64,
    #[serde(rename = "targetMs", alias = "target_ms")]
    pub target_ms: u32,
    #[serde(rename = "maxMs", alias = "max_ms")]
    pub max_ms: u32,
    pub sizes: Vec<usize>,
    pub quality: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Waypoint {
    pub step: usize,
    pub pos: [usize; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenerateMetrics {
    pub status: String,
    pub seed: u64,
    pub size: usize,
    pub quality: String,
    #[serde(rename = "qualityScore", alias = "quality_score")]
    pub quality_score: f64,
    #[serde(rename = "phaseTimings", alias = "phase_timings")]
    pub phase_timings: GeneratePhaseTimings,
    #[serde(rename = "totalMs", alias = "total_ms")]
    pub total_ms: f64,
    #[serde(rename = "targetMs", alias = "target_ms")]
    pub target_ms: u32,
    #[serde(rename = "maxMs", alias = "max_ms")]
    pub max_ms: u32,
    #[serde(rename = "degradationLevel", alias = "degradation_level")]
    pub degradation_level: u8,
    #[serde(rename = "candidateAttempts", alias = "candidate_attempts")]
    pub candidate_attempts: u32,
    #[serde(rename = "solverCalls", alias = "solver_calls")]
    pub solver_calls: u32,
    #[serde(rename = "uniqueChecks", alias = "unique_checks")]
    pub unique_checks: u32,
    pub cancelled: bool,
    pub fallback: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GeneratePhaseTimings {
    #[serde(rename = "candidateMs", alias = "candidate_ms")]
    pub candidate_ms: f64,
    #[serde(rename = "qualityMs", alias = "quality_ms")]
    pub quality_ms: f64,
    #[serde(rename = "totalMs", alias = "total_ms")]
    pub total_ms: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PuzzleResponse {
    #[serde(rename = "R", alias = "r")]
    pub r: usize,
    #[serde(rename = "C", alias = "c")]
    pub c: usize,
    pub obstacles: Vec<u8>,
    pub solution: Vec<[usize; 2]>,
    pub waypoints: Vec<Waypoint>,
    pub difficulty: String,
    pub metrics: GenerateMetrics,
}

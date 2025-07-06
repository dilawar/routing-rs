//! PCB Structure
//!
//! See the official documentation.

#[pyo3::pyclass]
#[derive(Debug, Default)]
pub struct Pcb {
    pub id: String,
    pub parser: Option<String>,
    pub capacitance_resolution: Option<String>,
    pub conductance_resolution: Option<String>,
    pub current_resolution: Option<String>,
    pub inductance_resolution: Option<String>,
    pub resistance_resolution: Option<String>,
    pub resolution: Option<String>,
    pub voltage_resolution: Option<String>,
    pub time_resolution: Option<String>,
    pub unit: Option<String>,
    pub stucture: Option<String>,
    pub placement: Option<String>,
    pub library: Option<String>,
    pub floor_plan: Option<String>,
    pub part_library: Option<String>,
    pub network: Option<String>,
    pub wiring: Option<String>,
    pub color: Option<String>,
}

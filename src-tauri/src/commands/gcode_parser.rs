use crate::error::AppError;
use crate::gcode_parser::ParsedProgram;

pub(crate) fn parse_gcode_inner(gcode: &str) -> Result<ParsedProgram, AppError> {
    Ok(crate::gcode_parser::parse_gcode(gcode))
}

#[tauri::command]
pub async fn parse_gcode(gcode: String) -> Result<ParsedProgram, AppError> {
    parse_gcode_inner(&gcode)
}

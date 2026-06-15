mod commands;

use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::import_file,
            commands::search,
            commands::execute_sql,
            commands::list_files,
            commands::list_table_aliases,
            commands::get_metadata,
            commands::get_sheet_sample,
            commands::get_sheet_data,
            commands::update_cell,
            commands::clear_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

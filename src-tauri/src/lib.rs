mod chunk;
mod commands;
mod db;
mod error;
mod extract;
mod models;
mod providers;
mod search;
mod vectordb;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            commands::init(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::dashboard,
            commands::get_settings,
            commands::save_provider,
            commands::test_provider,
            commands::set_retrieval_limit,
            commands::list_categories,
            commands::save_category,
            commands::delete_category,
            commands::list_cases,
            commands::get_case,
            commands::save_case,
            commands::delete_case,
            commands::list_people,
            commands::save_person,
            commands::delete_person,
            commands::case_roster,
            commands::link_person,
            commands::unlink_person,
            commands::list_documents,
            commands::get_document,
            commands::list_chunks,
            commands::import_documents,
            commands::reindex_document,
            commands::delete_document,
            commands::read_document_text,
            commands::list_conversations,
            commands::create_conversation,
            commands::delete_conversation,
            commands::list_messages,
            commands::send_message,
            commands::search_documents,
            commands::list_spreadsheets,
            commands::get_spreadsheet,
            commands::create_spreadsheet,
            commands::save_spreadsheet,
            commands::delete_spreadsheet,
            commands::list_events,
            commands::save_event,
            commands::delete_event,
            commands::case_tree,
        ])
        .run(tauri::generate_context!())
        .expect("error while running counsel");
}

pub mod config;
pub mod game;
pub mod main_menu;

pub use config::run as run_config;
pub use game::run as run_game;
pub use main_menu::run as run_main_menu;

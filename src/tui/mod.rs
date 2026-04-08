pub mod state;
pub mod tui_loop;
pub mod widgets;

pub use state::{SharedState, create_shared_state};
pub use widgets::render_app;

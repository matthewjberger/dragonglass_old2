mod app;
mod renderer;

use app::App;
use log::{error, info};

fn main() {
    match App::run() {
        Ok(_) => info!("Program exited successfully."),
        Err(error) => error!("Program failed: {:?}", error),
    }
}

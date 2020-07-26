mod app;
mod camera;
mod gui;
mod input;
mod renderer;
mod system;

use anyhow::Result;
use app::App;

fn main() -> Result<()> {
    App::run()
}

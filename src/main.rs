mod app;
mod camera;
mod gui;
mod input;
mod renderer;

use anyhow::Result;
use app::App;

fn main() -> Result<()> {
    App::run()
}

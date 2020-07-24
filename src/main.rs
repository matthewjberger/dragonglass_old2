mod app;
mod camera;
mod renderer;
mod script;

use anyhow::Result;
use app::App;

fn main() -> Result<()> {
    App::run()
}

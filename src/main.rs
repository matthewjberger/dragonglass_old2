mod app;
mod camera;
mod renderer;

use anyhow::Result;
use app::App;

fn main() -> Result<()> {
    App::run()
}

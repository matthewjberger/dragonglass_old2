mod app;
mod camera;
mod renderer;

use app::{App, Error};

fn main() -> Result<(), Error> {
    App::run()
}

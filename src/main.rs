mod app;
mod renderer;

use app::{App, Error};

fn main() -> Result<(), Error> {
    App::run()
}

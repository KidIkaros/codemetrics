// @sensitive
let my_credential = load_secret();
fn f() { log::info!("{}", my_credential); }

mod utils;
mod parser;

use crate::utils::helper;
use crate::parser::parse;

fn main() {
    helper();
    parse("hello");
}

use std::{mem::forget, sync::Arc};

use async_runtime::Runtime;

fn main() {
    let rt = Runtime;
    rt.run(async {
        println!("async");
    });
}


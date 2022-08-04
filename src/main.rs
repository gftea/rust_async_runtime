use std::future::Future;

use async_runtime::net::AsyncTcpStream;
use async_runtime::Runtime;

async fn f1() {
    let mut stream = AsyncTcpStream::connect("127.0.0.1:8080");

    let mut buf = vec![0; 100];
    // `read` will return future that is awaitable
    let n = stream.read(&mut buf).await;

    // Our own test server  send a packet to us after connected
    // we should get it after `read` is driven to completion by runtime
    println!(
        "connection 1 from server: {:?} | {}:{} | ",
        String::from_utf8(buf[0..n].into()),
        file!(),
        line!(),
    );
    stream.close();
}
async fn f2() {
    let mut stream = AsyncTcpStream::connect("127.0.0.1:8080");

    let mut buf = vec![0; 100];
    // `read` will return future that is awaitable
    let n = stream.read(&mut buf).await;

    // Our own test server  send a packet to us after connected
    // we should get it after `read` is driven to completion by runtime
    println!(
        "connection 2 from server: {:?} | {}:{} | ",
        String::from_utf8(buf[0..n].into()),
        file!(),
        line!(),
    );
    stream.close();
}
fn main() {
    let rt = Runtime {};

    let mut futures: Vec<Box<dyn Future<Output=()> + Send + 'static>> = vec![];
    futures.push(Box::new(f1()));
    futures.push(Box::new(f2()));

    rt.run(futures);
}

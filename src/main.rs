use async_runtime::net::AsyncTcpStream;
use async_runtime::Runtime;

fn main() {
    let rt = Runtime;
    rt.run(async {
        let mut stream = AsyncTcpStream::connect("127.0.0.1:8080");

        let mut buf = vec![0; 100];
        // `read` will return future that is awaitable
        let n = stream.read(&mut buf).await;

        // Our own test server  send a packet to us after connected
        // we should get it after `read` is driven to completion by runtime
        println!(
            "from server: {:?} | {}:{} | ",
            String::from_utf8(buf[0..n].into()),
            file!(),
            line!(),
        );
        stream.close();
    });
}

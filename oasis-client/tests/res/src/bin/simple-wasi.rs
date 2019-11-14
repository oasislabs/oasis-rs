use std::io::Read as _;

fn read_stdin() -> Vec<u8> {
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).unwrap();
    buf
}

#[no_mangle]
extern "C" fn _oasis_deploy() {
    std::fs::write("cloud-init", &read_stdin()).unwrap();
}

fn main() {
    if String::from_utf8(read_stdin()).unwrap() == "hello, service!" {
        print!(
            "hello, client! your initial data was {}",
            std::fs::read_to_string("cloud-init").unwrap()
        );
    }
}

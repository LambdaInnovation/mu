extern crate mu;

fn main() {
    let runtime = mu::RuntimeBuilder::new("Example window").build();
    runtime.start();
}
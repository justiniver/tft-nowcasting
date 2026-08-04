use std::env;

fn main() {
    let n = env::args()
        .nth(1)
        .expect("Usage: cargo run --example scrap -- <number>")
        .parse::<u64>()
        .expect("Please provide a positive integer");
    let sequence = collatz(n);
    println!("{sequence:?}");
}

fn collatz(n: u64) -> Vec<u64> {
    assert!(n > 0, "Collatz requires a positive integer");
    let mut sequence = vec![n];

    if n == 1 {
        return sequence;
    }

    let next = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };

    sequence.extend(collatz(next));
    sequence
}

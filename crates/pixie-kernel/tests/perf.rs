//! Where the object-access cost goes. Run with
//! `cargo test -p pixie-kernel --release --test perf -- --ignored --nocapture`.
use pixie_kernel::*;
use std::time::Instant;

struct Cell {
    v: i64,
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore = "a benchmark, not an assertion"]
fn where_the_object_access_cost_goes() {
    const N: i64 = 20_000_000;

    let mut plain = Cell { v: 0 };
    let t = Instant::now();
    for i in 0..N {
        plain.v = std::hint::black_box(plain.v + i);
    }
    let floor = ms(t);

    let mut w = World::new();
    let h = w.insert(Cell { v: 0 });
    let t = Instant::now();
    let mut acc = 0i64;
    for _ in 0..N {
        acc += std::hint::black_box(w.get(h).v);
    }
    let read = ms(t);
    std::hint::black_box(acc);

    let t = Instant::now();
    for i in 0..N {
        let cur = w.get(h).v;
        w.get_mut(h).v = cur + i;
    }
    let rmw = ms(t);

    let t = Instant::now();
    for i in 0..N {
        let cur = w.get(h).v;
        if cur != cur + i {
            w.get_mut(h).v = cur + i;
            w.notify_changed(h.erase(), 1);
        }
    }
    let full = ms(t);

    // 4. What the emitter ACTUALLY produces: the getter reads the
    //    field, then the setter reads it AGAIN for its dirty check
    //    before writing. Three lookups per assignment, not two.
    let t = Instant::now();
    for i in 0..N {
        let cur = w.get(h).v; // the getter
        let next = cur + i;
        if w.get(h).v != next {
            // the setter's dirty check re-reads
            w.get_mut(h).v = next;
            w.notify_changed(h.erase(), 1);
        }
    }
    let emitted = ms(t);

    // 5. The setter with one access instead of two (§8.50).
    let t = Instant::now();
    for i in 0..N {
        let cur = w.get(h).v;
        let next = cur + i;
        let slot = w.get_mut(h);
        if slot.v != next {
            slot.v = next;
            w.notify_changed(h.erase(), 1);
        }
    }
    let one_access = ms(t);

    // 6. What `notify_changed` costs on its own: it hashes the target
    //    to ask whether anyone is listening.
    let t = Instant::now();
    for _ in 0..N {
        w.notify_changed(h.erase(), 1);
    }
    let notify = ms(t);

    // 7. The same, but with the World in the state a real app is in:
    //    a view is mounted and stores are connected, so the "is
    //    anyone listening" set is NOT empty and the lookup actually
    //    hashes. An empty set short-circuits, which is why a bench
    //    that never connects anything flatters itself.
    let other = w.insert(Cell { v: 0 });
    w.connect(other.erase(), 1, std::rc::Rc::new(|_| {}));
    let t = Instant::now();
    for i in 0..N {
        let cur = w.get(h).v;
        let next = cur + i;
        let slot = w.get_mut(h);
        if slot.v != next {
            slot.v = next;
            w.notify_changed(h.erase(), 1);
        }
    }
    let realistic = ms(t);

    let per = |x: f64| x * 1e6 / N as f64;
    println!("\n  N = {N} iterations, release\n");
    println!("  plain Rust struct field     {floor:7.0} ms   {:5.2} ns/iter", per(floor));
    println!("  World::get                  {read:7.0} ms   {:5.2} ns/iter", per(read));
    println!("  get + get_mut               {rmw:7.0} ms   {:5.2} ns/iter", per(rmw));
    println!("  ... + dirty check + notify  {full:7.0} ms   {:5.2} ns/iter", per(full));
    println!("  the emitted shape (3 reads) {emitted:7.0} ms   {:5.2} ns/iter", per(emitted));
    println!("  ... with one access (§8.50)  {one_access:7.0} ms   {:5.2} ns/iter", per(one_access));
    println!("  notify_changed alone        {notify:7.0} ms   {:5.2} ns/iter", per(notify));
    println!("  ... with listeners present  {realistic:7.0} ms   {:5.2} ns/iter", per(realistic));
    println!();
}

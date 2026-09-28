//! `Vec` indexing and `len`: in-range reads and a walk by index agree with
//! Rust, and an out-of-range index panics on both sides with the same message.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod vecs = "fixtures/vec.rs");

const SOURCE: &str = vecs::SOURCE;

#[test]
fn generated_vec_reads_match_rust() {
    let samples = [vec![], vec![0], vec![1, 2], vec![-3, 4, 5, 6]];
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for xs in &samples {
            cases.push(case!(vecs::len(xs.clone())));
            cases.push(case!(vecs::sum_from(xs.as_slice(), 0usize)));
            if xs.len() > 1 {
                cases.push(case!(vecs::at(xs.clone(), 0usize)));
                cases.push(case!(vecs::at(xs.clone(), 1usize)));
                cases.push(case!(vecs::second(xs.clone())));
            } else if xs.len() == 1 {
                cases.push(case!(vecs::at(xs.clone(), 0usize)));
            }
        }
        cases.push(case!(vecs::at(vec![1i32, 2, 3], 5usize)));
        cases.push(case!(vecs::at(vec![0i32; 0], 0usize)));
        cases
    });
    support::assert_equivalent("vecs", SOURCE, &cases);
}

#[test]
fn out_of_range_index_uses_rusts_panic_message() {
    let payload = std::panic::catch_unwind(|| {
        let xs = vec![1, 2, 3];
        let _ = xs[5];
    })
    .expect_err("index 5 panics");
    let rust = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .expect("panic payload");
    assert_eq!(rust, "index out of bounds: the len is 3 but the index is 5");

    let krate = purecrate_syntax::parse_source("vecs", SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let dir = std::env::temp_dir().join(format!("purecrate-vec-msg-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).ok();
    }
    for file in purecrate_pack::assemble(&typed).files {
        let path = dir.join(purecrate_pack::disk_path(&file.stem));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.source).unwrap();
    }
    std::fs::write(
        dir.join("driver.ts"),
        "import { at } from \"./src/at.ts\";\ntry { at([1, 2, 3], 5); } catch (e) { console.log(e.message); }\n",
    )
    .unwrap();
    support::link_purecrate(&dir);
    let output = std::process::Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let ts = String::from_utf8(output.stdout).unwrap();
    assert_eq!(ts.trim(), rust);
    std::fs::remove_dir_all(&dir).ok();
}

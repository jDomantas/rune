use rune_tests::*;

#[test]
fn test_float_fns() {
    let n: f64 = rune! {
        pub fn main() {
            1.0.min(2.0)
        }
    };
    assert_eq!(n, 1.0);

    let n: f64 = rune! {
        pub fn main() {
            std::float::min(1.0, 2.0)
        }
    };
    assert_eq!(n, 1.0);

    let n: f64 = rune! {
        pub fn main() {
            1.0.max(2.0)
        }
    };
    assert_eq!(n, 2.0);

    let n: f64 = rune! {
        pub fn main() {
            std::float::max(1.0, 2.0)
        }
    };
    assert_eq!(n, 2.0);

    let n: f64 = rune! {
        pub fn main() {
            (-10.0).abs()
        }
    };
    assert_eq!(n, 10.0);

    let n: f64 = rune! {
        pub fn main() {
            std::float::abs(-10.0)
        }
    };
    assert_eq!(n, 10.0);

    let n: f64 = rune! {
        pub fn main() {
            (12.0).powf(3.0)
        }
    };
    assert_eq!(n, 1728.0);

    let n: f64 = rune! {
        pub fn main() {
            std::float::powf(12.0, 3.0)
        }
    };
    assert_eq!(n, 1728.0);
}

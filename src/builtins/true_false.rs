//! true, false — always succeed / always fail.

pub fn true_cmd() -> i32 {
    0
}

pub fn false_cmd() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_true() {
        assert_eq!(true_cmd(), 0);
    }

    #[test]
    fn test_false() {
        assert_eq!(false_cmd(), 1);
    }
}

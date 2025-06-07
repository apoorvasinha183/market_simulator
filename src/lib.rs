pub fn placeholder_function() -> String {
    "The simulator is ready!".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = placeholder_function();
        assert_eq!(result, "The simulator is ready!");
    }
}

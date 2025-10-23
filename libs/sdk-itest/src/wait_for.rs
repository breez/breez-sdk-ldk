#[macro_export]
macro_rules! wait_for {
    ($condition:expr) => {
        while !$condition {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    };
}
